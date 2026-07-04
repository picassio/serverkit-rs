//! sk-system — host metrics and system info.
//!
//! Ports `app/services/system_service.py` (psutil) and
//! `app/services/resource_tier_service.py` to sysinfo + /proc.
//! Output shapes must match the Flask JSON exactly — the dashboard
//! consumes them field-by-field.

pub mod fmt;
pub mod processes;

use chrono::{Local, TimeZone, Utc};
use serde_json::{json, Value};
use std::net::UdpSocket;
use std::sync::OnceLock;
use sysinfo::{CpuRefreshKind, Disks, Networks, RefreshKind, System};
use tokio::sync::Mutex;

use fmt::{format_bytes, format_uptime};

/// Shared sysinfo handle. CPU usage needs two refreshes separated by an
/// interval; keeping one System avoids re-priming per request.
static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();

fn system() -> &'static Mutex<System> {
    SYSTEM.get_or_init(|| {
        Mutex::new(System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(sysinfo::MemoryRefreshKind::everything()),
        ))
    })
}

/// `SystemService.get_cpu_metrics()`
pub async fn cpu_metrics() -> Value {
    let mut sys = system().lock().await;
    sys.refresh_cpu_usage();
    tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    sys.refresh_cpu_usage();

    let per_cpu: Vec<Value> = sys
        .cpus()
        .iter()
        .map(|c| json!(round1(c.cpu_usage() as f64)))
        .collect();
    let global = round1(sys.global_cpu_usage() as f64);
    let freq_current = sys.cpus().first().map(|c| c.frequency()).unwrap_or(0);

    json!({
        "percent": global,
        "count_physical": sys.physical_core_count().unwrap_or(sys.cpus().len()),
        "count_logical": sys.cpus().len(),
        "frequency": {
            // psutil reports MHz; sysinfo frequency() is MHz too.
            "current": freq_current,
            "min": 0,
            "max": 0
        },
        "per_cpu": per_cpu
    })
}

/// `SystemService.get_memory_metrics()`
pub async fn memory_metrics() -> Value {
    let mut sys = system().lock().await;
    sys.refresh_memory();

    let total = sys.total_memory();
    let available = sys.available_memory();
    let used = sys.used_memory();
    let cached = proc_meminfo_cached().unwrap_or(0);
    let percent = if total > 0 {
        round1((total - available) as f64 / total as f64 * 100.0)
    } else {
        0.0
    };

    let swap_total = sys.total_swap();
    let swap_used = sys.used_swap();
    let swap_free = sys.free_swap();
    let swap_percent = if swap_total > 0 {
        round1(swap_used as f64 / swap_total as f64 * 100.0)
    } else {
        0.0
    };

    json!({
        "ram": {
            "total": total,
            "available": available,
            "used": used,
            "cached": cached,
            "percent": percent,
            "total_human": format_bytes(total),
            "available_human": format_bytes(available),
            "used_human": format_bytes(used),
            "cached_human": format_bytes(cached)
        },
        "swap": {
            "total": swap_total,
            "used": swap_used,
            "free": swap_free,
            "percent": swap_percent,
            "total_human": format_bytes(swap_total),
            "used_human": format_bytes(swap_used)
        }
    })
}

/// `SystemService.get_disk_metrics()`
pub fn disk_metrics() -> Value {
    let disks = Disks::new_with_refreshed_list();
    let partitions: Vec<Value> = disks
        .iter()
        .map(|d| {
            let total = d.total_space();
            let free = d.available_space();
            let used = total.saturating_sub(free);
            let percent = if total > 0 {
                round1(used as f64 / total as f64 * 100.0)
            } else {
                0.0
            };
            json!({
                "device": d.name().to_string_lossy(),
                "mountpoint": d.mount_point().to_string_lossy(),
                "fstype": d.file_system().to_string_lossy(),
                "total": total,
                "used": used,
                "free": free,
                "percent": percent,
                "total_human": format_bytes(total),
                "used_human": format_bytes(used),
                "free_human": format_bytes(free)
            })
        })
        .collect();

    // TODO(P1): global disk I/O counters (/proc/diskstats). Flask emits null
    // when psutil fails, so null is a valid value here.
    json!({ "partitions": partitions, "io": Value::Null })
}

/// `SystemService.get_network_metrics()`
pub fn network_metrics() -> Value {
    let networks = Networks::new_with_refreshed_list();

    let mut bytes_sent: u64 = 0;
    let mut bytes_recv: u64 = 0;
    let mut packets_sent: u64 = 0;
    let mut packets_recv: u64 = 0;
    let mut interfaces = Vec::new();

    for (name, data) in networks.iter() {
        bytes_sent += data.total_transmitted();
        bytes_recv += data.total_received();
        packets_sent += data.total_packets_transmitted();
        packets_recv += data.total_packets_received();
        let addresses: Vec<Value> = data
            .ip_networks()
            .iter()
            .map(|ip| {
                json!({
                    "family": if ip.addr.is_ipv4() { "AddressFamily.AF_INET" } else { "AddressFamily.AF_INET6" },
                    "address": ip.addr.to_string(),
                    "netmask": Value::Null
                })
            })
            .collect();
        interfaces.push(json!({
            "name": name,
            "is_up": true, // sysinfo lists active interfaces only
            "speed": 0,
            "addresses": addresses
        }));
    }

    let io = json!({
        "bytes_sent": bytes_sent,
        "bytes_recv": bytes_recv,
        "packets_sent": packets_sent,
        "packets_recv": packets_recv,
        "bytes_sent_human": format_bytes(bytes_sent),
        "bytes_recv_human": format_bytes(bytes_recv),
    });

    json!({
        "io": io,
        // Flat duplicates kept for frontend backwards compatibility (Flask does the same)
        "bytes_sent": bytes_sent,
        "bytes_recv": bytes_recv,
        "packets_sent": packets_sent,
        "packets_recv": packets_recv,
        "bytes_sent_human": format_bytes(bytes_sent),
        "bytes_recv_human": format_bytes(bytes_recv),
        "interfaces": interfaces
    })
}

/// `SystemService.get_load_average()`
pub fn load_average() -> Value {
    let load = System::load_average();
    json!({
        "1min": round2(load.one),
        "5min": round2(load.five),
        "15min": round2(load.fifteen)
    })
}

/// `SystemService.get_system_info()`
pub fn system_info() -> Value {
    let boot_ts = System::boot_time() as i64;
    let boot_local = Local
        .timestamp_opt(boot_ts, 0)
        .single()
        .unwrap_or_else(Local::now);
    let uptime_secs = System::uptime();

    let kernel = System::kernel_version().unwrap_or_else(|| "Unknown".into());
    let cpu_model = proc_cpu_model().unwrap_or_else(|| "Unknown".into());
    let arch = System::cpu_arch();

    json!({
        "platform": System::name().unwrap_or_else(|| "Linux".into()),
        "platform_release": kernel,
        "platform_version": System::os_version().unwrap_or_default(),
        "architecture": arch,
        "hostname": System::host_name().unwrap_or_else(|| "unknown".into()),
        "processor": cpu_model,
        "python_version": Value::Null, // Rust backend — no Python runtime
        "boot_time": boot_local.naive_local().format("%Y-%m-%dT%H:%M:%S").to_string(),
        "uptime_seconds": uptime_secs,
        "uptime_human": format_uptime(uptime_secs),
        "ip_address": primary_ip(),
        "kernel": kernel,
        "cpu": { "model": cpu_model, "architecture": arch }
    })
}

/// `SystemService.get_server_time()`
pub fn server_time() -> Value {
    let now = Local::now();
    let utc = Utc::now();
    let offset_seconds = now.offset().local_minus_utc() as i64;
    let offset_hours = offset_seconds / 3600;
    let offset_minutes = (offset_seconds.abs() % 3600) / 60;
    let tz_id = std::fs::read_to_string("/etc/timezone")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    json!({
        "current_time": now.naive_local().format("%Y-%m-%dT%H:%M:%S%.6f").to_string(),
        "current_time_formatted": now.format("%Y-%m-%d %H:%M:%S").to_string(),
        "utc_time": utc.naive_utc().format("%Y-%m-%dT%H:%M:%S%.6f").to_string(),
        "timezone_name": tz_id.clone().unwrap_or_else(|| "UTC".into()),
        "timezone_id": tz_id,
        "utc_offset": format!("UTC{offset_hours:+}:{offset_minutes:02}"),
        "utc_offset_seconds": offset_seconds
    })
}

/// `SystemService.get_all_metrics()`
pub async fn all_metrics() -> Value {
    json!({
        "cpu": cpu_metrics().await,
        "memory": memory_metrics().await,
        "disk": disk_metrics(),
        "network": network_metrics(),
        "load_average": load_average(),
        "system": system_info(),
        "time": server_time(),
        "timestamp": Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S%.6f").to_string()
    })
}

/// `ResourceTierService.get_tier_info()` — lite (<2 cores or <2GB),
/// standard (2-3 cores, 2-4GB), performance (4+ cores, >4GB).
pub async fn resource_tier() -> Value {
    let mut sys = system().lock().await;
    sys.refresh_memory();
    let cpu_cores = sys
        .physical_core_count()
        .unwrap_or_else(|| sys.cpus().len().max(1));
    let ram_bytes = sys.total_memory();
    let ram_gb = (ram_bytes as f64 / (1024f64.powi(3)) * 100.0).round() / 100.0;

    let tier = if cpu_cores >= 4 && ram_gb > 4.0 {
        "performance"
    } else if cpu_cores >= 2 && ram_gb >= 2.0 {
        "standard"
    } else {
        "lite"
    };

    json!({
        "tier": tier,
        "specs": { "cpu_cores": cpu_cores, "ram_gb": ram_gb, "ram_bytes": ram_bytes },
        "features": { "wordpress_create": tier != "lite" },
        "cached": false
    })
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// `SystemService._get_primary_ip()` — UDP-connect trick, no packets sent.
fn primary_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "Unknown".into())
}

fn proc_cpu_model() -> Option<String> {
    let content = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    content
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
}

fn proc_meminfo_cached() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    content
        .lines()
        .find(|l| l.starts_with("Cached:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|kb| kb.parse::<u64>().ok())
        .map(|kb| kb * 1024)
}
