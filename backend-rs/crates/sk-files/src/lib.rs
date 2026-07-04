//! sk-files — File Manager operations, ported from `app/services/file_service.py`.
//!
//! All functions are synchronous (std::fs); route handlers wrap them in
//! `spawn_blocking`. Response shapes match `FileService` field-for-field.

use chrono::{DateTime, Local};
use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

/// `FileService.ALLOWED_ROOTS` (SERVERKIT_DIR ≈ /opt/serverkit covered by /opt).
pub const ALLOWED_ROOTS: &[&str] = &["/home", "/var/www", "/opt", "/srv", "/var/log"];

/// `FileService.MAX_EDIT_SIZE` (5MB)
pub const MAX_EDIT_SIZE: u64 = 5 * 1024 * 1024;
/// `FileService.MAX_UPLOAD_SIZE` (100MB)
pub const MAX_UPLOAD_SIZE: u64 = 100 * 1024 * 1024;

const EDITABLE_EXTENSIONS: &[&str] = &[
    "txt",
    "md",
    "json",
    "xml",
    "yml",
    "yaml",
    "ini",
    "conf",
    "cfg",
    "py",
    "js",
    "jsx",
    "ts",
    "tsx",
    "html",
    "css",
    "less",
    "scss",
    "php",
    "rb",
    "go",
    "rs",
    "java",
    "c",
    "cpp",
    "h",
    "sh",
    "bash",
    "sql",
    "env",
    "htaccess",
    "gitignore",
    "dockerfile",
    "toml",
];

const DENIED: &str = "Access denied: path not in allowed directories";

fn err(msg: impl Into<String>) -> Value {
    json!({ "success": false, "error": msg.into() })
}

/// `os.path.realpath` equivalent that also works for not-yet-existing paths:
/// canonicalize the deepest existing ancestor, then re-append the remainder.
fn realpath(path: &str) -> PathBuf {
    let p = Path::new(path);
    let mut existing = p.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    while !existing.exists() {
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                tail.push(name.to_os_string());
                existing = parent.to_path_buf();
            }
            _ => break,
        }
    }
    let mut real = fs::canonicalize(&existing).unwrap_or(existing);
    for part in tail.iter().rev() {
        real.push(part);
    }
    // normalize any remaining ".." lexically (defense in depth)
    let mut clean = PathBuf::new();
    for c in real.components() {
        match c {
            Component::ParentDir => {
                clean.pop();
            }
            Component::CurDir => {}
            other => clean.push(other.as_os_str()),
        }
    }
    clean
}

/// `FileService.is_path_allowed`
pub fn is_path_allowed(path: &str) -> bool {
    let real = realpath(path);
    let real = real.to_string_lossy();
    ALLOWED_ROOTS.iter().any(|root| real.starts_with(root))
}

fn format_size(size: u64) -> String {
    if size == 0 {
        return "0 B".into();
    }
    let mut val = size as f64;
    for unit in ["B", "KB", "MB", "GB", "TB"] {
        if val < 1024.0 {
            return format!("{val:.1} {unit}");
        }
        val /= 1024.0;
    }
    format!("{val:.1} PB")
}

fn isoformat(t: std::time::SystemTime) -> String {
    DateTime::<Local>::from(t)
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%S%.6f")
        .to_string()
}

/// `stat.filemode` — e.g. `drwxr-xr-x`.
fn filemode(meta: &fs::Metadata) -> String {
    let mode = meta.permissions().mode();
    let kind = if meta.is_dir() {
        'd'
    } else if meta.file_type().is_symlink() {
        'l'
    } else {
        '-'
    };
    let mut s = String::with_capacity(10);
    s.push(kind);
    for shift in [6, 3, 0] {
        let bits = (mode >> shift) & 0o7;
        s.push(if bits & 0o4 != 0 { 'r' } else { '-' });
        s.push(if bits & 0o2 != 0 { 'w' } else { '-' });
        s.push(if bits & 0o1 != 0 { 'x' } else { '-' });
    }
    s
}

fn lookup_name(file: &str, id: u32) -> Option<String> {
    let content = fs::read_to_string(file).ok()?;
    for line in content.lines() {
        let mut parts = line.split(':');
        let name = parts.next()?;
        parts.next(); // password field
        if let Some(entry_id) = parts.next().and_then(|s| s.parse::<u32>().ok()) {
            if entry_id == id {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn is_editable(path: &Path, meta: &fs::Metadata) -> bool {
    if meta.is_dir() || meta.len() > MAX_EDIT_SIZE {
        return false;
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => EDITABLE_EXTENSIONS.contains(&ext.to_lowercase().as_str()),
        None => fs::read(path)
            .map(|b| std::str::from_utf8(&b[..b.len().min(1024)]).is_ok())
            .unwrap_or(false),
    }
}

/// Non-recursive directory size (immediate files only) — `_get_dir_size`.
fn dir_size(path: &Path) -> u64 {
    fs::read_dir(path)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

/// `FileService.get_file_info`
pub fn file_info(path: &str) -> Option<Value> {
    let p = Path::new(path);
    let meta = fs::symlink_metadata(p).ok()?;
    let is_link = meta.file_type().is_symlink();
    // follow the link for the rest (Flask's os.stat follows)
    let meta = fs::metadata(p).unwrap_or(meta);
    let is_dir = meta.is_dir();

    let owner = lookup_name("/etc/passwd", meta.uid()).unwrap_or_else(|| meta.uid().to_string());
    let group = lookup_name("/etc/group", meta.gid()).unwrap_or_else(|| meta.gid().to_string());
    let mime_type = if is_dir {
        None
    } else {
        mime_guess::from_path(p).first().map(|m| m.to_string())
    };

    let readable = access(path, 4); // R_OK
    let writable = access(path, 2); // W_OK
    let executable = access(path, 1); // X_OK

    Some(json!({
        "name": p.file_name().map(|n| n.to_string_lossy().into_owned())
                 .unwrap_or_else(|| path.to_string()),
        "path": path,
        "is_dir": is_dir,
        "is_file": !is_dir,
        "is_link": is_link,
        "size": if is_dir { dir_size(p) } else { meta.len() },
        "size_human": format_size(meta.len()),
        "modified": meta.modified().ok().map(isoformat),
        "created": meta.created().ok().or(meta.modified().ok()).map(isoformat),
        "accessed": meta.accessed().ok().map(isoformat),
        "permissions": filemode(&meta),
        "permissions_octal": format!("{:03o}", meta.mode() & 0o777),
        "owner": owner,
        "group": group,
        "mime_type": mime_type,
        "is_editable": is_editable(p, &meta),
        "is_readable": readable,
        "is_writable": writable,
        "is_executable": executable,
    }))
}

/// `os.access` equivalent (checks against the effective uid/gid).
fn access(path: &str, amode: i32) -> bool {
    extern "C" {
        fn access(pathname: *const std::os::raw::c_char, mode: i32) -> i32;
    }
    let Ok(c_path) = std::ffi::CString::new(path) else {
        return false;
    };
    // SAFETY: c_path is a valid NUL-terminated string for the call duration
    unsafe { access(c_path.as_ptr(), amode) == 0 }
}

/// `FileService.list_directory`
pub fn list_directory(path: &str, show_hidden: bool) -> Value {
    if !is_path_allowed(path) {
        return err(DENIED);
    }
    let p = Path::new(path);
    if !p.exists() {
        return err("Directory not found");
    }
    if !p.is_dir() {
        return err("Not a directory");
    }

    let entries_iter = match fs::read_dir(p) {
        Ok(it) => it,
        Err(e) => return err(e.to_string()),
    };

    let mut entries: Vec<Value> = entries_iter
        .flatten()
        .filter(|e| show_hidden || !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| file_info(&e.path().to_string_lossy()))
        .collect();

    // dirs first, then case-insensitive name
    entries.sort_by(|a, b| {
        let ad = !a["is_dir"].as_bool().unwrap_or(false);
        let bd = !b["is_dir"].as_bool().unwrap_or(false);
        ad.cmp(&bd).then(
            a["name"]
                .as_str()
                .unwrap_or("")
                .to_lowercase()
                .cmp(&b["name"].as_str().unwrap_or("").to_lowercase()),
        )
    });

    let parent = p.parent().map(|pp| pp.to_string_lossy().into_owned());
    let parent = parent.filter(|pp| is_path_allowed(pp));

    json!({
        "success": true,
        "path": path,
        "parent": parent,
        "total": entries.len(),
        "entries": entries,
    })
}

/// `FileService.read_file`
pub fn read_file(path: &str) -> Value {
    if !is_path_allowed(path) {
        return err(DENIED);
    }
    let p = Path::new(path);
    if !p.exists() {
        return err("File not found");
    }
    if p.is_dir() {
        return err("Cannot read directory");
    }
    let size = p.metadata().map(|m| m.len()).unwrap_or(0);
    if size > MAX_EDIT_SIZE {
        return err(format!(
            "File too large to edit ({}). Maximum is {}",
            format_size(size),
            format_size(MAX_EDIT_SIZE)
        ));
    }
    match fs::read(p) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(content) => json!({
                "success": true,
                "path": path,
                "content": content,
                "encoding": "utf-8",
                "size": size,
                "is_binary": false,
            }),
            Err(_) => json!({
                "success": false,
                "error": "Binary file cannot be edited",
                "is_binary": true,
            }),
        },
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => err("Permission denied"),
        Err(e) => err(e.to_string()),
    }
}

/// `FileService.write_file` — creates a `.bak` alongside by default.
pub fn write_file(path: &str, content: &str, create_backup: bool) -> Value {
    if !is_path_allowed(path) {
        return err(DENIED);
    }
    let p = Path::new(path);
    if create_backup && p.exists() {
        let _ = fs::copy(p, format!("{path}.bak"));
    }
    match fs::write(p, content) {
        Ok(_) => json!({ "success": true, "path": path, "size": content.len() }),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => err("Permission denied"),
        Err(e) => err(e.to_string()),
    }
}

/// `FileService.create_file`
pub fn create_file(path: &str, content: &str) -> Value {
    if !is_path_allowed(path) {
        return err(DENIED);
    }
    if Path::new(path).exists() {
        return err("File already exists");
    }
    match fs::write(path, content) {
        Ok(_) => json!({ "success": true, "path": path }),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => err("Permission denied"),
        Err(e) => err(e.to_string()),
    }
}

/// `FileService.create_directory`
pub fn create_directory(path: &str) -> Value {
    if !is_path_allowed(path) {
        return err(DENIED);
    }
    if Path::new(path).exists() {
        return err("Directory already exists");
    }
    match fs::create_dir_all(path) {
        Ok(_) => json!({ "success": true, "path": path }),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => err("Permission denied"),
        Err(e) => err(e.to_string()),
    }
}

/// `FileService.delete`
pub fn delete(path: &str) -> Value {
    if !is_path_allowed(path) {
        return err(DENIED);
    }
    let p = Path::new(path);
    if !p.exists() {
        return err("Path not found");
    }
    let result = if p.is_dir() {
        fs::remove_dir_all(p)
    } else {
        fs::remove_file(p)
    };
    match result {
        Ok(_) => json!({ "success": true, "path": path }),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => err("Permission denied"),
        Err(e) => err(e.to_string()),
    }
}

/// `FileService.rename` — new_name must not contain separators.
pub fn rename(path: &str, new_name: &str) -> Value {
    if !is_path_allowed(path) {
        return err(DENIED);
    }
    let p = Path::new(path);
    if !p.exists() {
        return err("Path not found");
    }
    if new_name.contains('/') || new_name.contains('\\') {
        return err("Invalid filename: path separators not allowed");
    }
    let new_path = p.with_file_name(new_name);
    let new_path_str = new_path.to_string_lossy().into_owned();
    if !is_path_allowed(&new_path_str) {
        return err("Access denied: target path not allowed");
    }
    if new_path.exists() {
        return err("Target already exists");
    }
    match fs::rename(p, &new_path) {
        Ok(_) => json!({ "success": true, "old_path": path, "new_path": new_path_str }),
        Err(e) => err(e.to_string()),
    }
}

fn copy_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dest)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            copy_recursive(&entry.path(), &dest.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(src, dest).map(|_| ())
    }
}

/// `FileService.copy`
pub fn copy(src: &str, dest: &str) -> Value {
    if !is_path_allowed(src) || !is_path_allowed(dest) {
        return err(DENIED);
    }
    let s = Path::new(src);
    if !s.exists() {
        return err("Source not found");
    }
    if Path::new(dest).exists() {
        return err("Destination already exists");
    }
    match copy_recursive(s, Path::new(dest)) {
        Ok(_) => json!({ "success": true, "src": src, "dest": dest }),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => err("Permission denied"),
        Err(e) => err(e.to_string()),
    }
}

/// `FileService.move`
pub fn move_path(src: &str, dest: &str) -> Value {
    if !is_path_allowed(src) || !is_path_allowed(dest) {
        return err(DENIED);
    }
    let s = Path::new(src);
    if !s.exists() {
        return err("Source not found");
    }
    if Path::new(dest).exists() {
        return err("Destination already exists");
    }
    match fs::rename(s, dest) {
        Ok(_) => json!({ "success": true, "src": src, "dest": dest }),
        Err(_) => {
            // cross-device: copy then remove (shutil.move semantics)
            match copy_recursive(s, Path::new(dest)) {
                Ok(_) => {
                    let _ = if s.is_dir() {
                        fs::remove_dir_all(s)
                    } else {
                        fs::remove_file(s)
                    };
                    json!({ "success": true, "src": src, "dest": dest })
                }
                Err(e) => err(e.to_string()),
            }
        }
    }
}

/// `FileService.change_permissions` — octal string "000".."777".
pub fn chmod(path: &str, mode: &str) -> Value {
    if !is_path_allowed(path) {
        return err(DENIED);
    }
    let p = Path::new(path);
    if !p.exists() {
        return err("Path not found");
    }
    let Ok(bits) = u32::from_str_radix(mode, 8) else {
        return err("Invalid permission mode");
    };
    if bits > 0o777 {
        return err("Invalid permission mode. Must be between 000 and 777.");
    }
    match fs::set_permissions(p, fs::Permissions::from_mode(bits)) {
        Ok(_) => json!({ "success": true, "path": path, "mode": mode }),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => err("Permission denied"),
        Err(e) => err(e.to_string()),
    }
}

/// `FileService.search` — case-insensitive substring walk, hidden dirs skipped.
pub fn search(directory: &str, pattern: &str, max_results: usize) -> Value {
    if !is_path_allowed(directory) {
        return err(DENIED);
    }
    if !Path::new(directory).is_dir() {
        return err("Directory not found");
    }

    let needle = pattern.to_lowercase();
    let mut results = Vec::new();
    let mut stack = vec![PathBuf::from(directory)];
    let mut truncated = false;

    'walk: while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir && !name.starts_with('.') {
                stack.push(entry.path());
            }
            if name.to_lowercase().contains(&needle) {
                let full = entry.path().to_string_lossy().into_owned();
                if is_path_allowed(&full) {
                    if let Some(info) = file_info(&full) {
                        results.push(info);
                        if results.len() >= max_results {
                            truncated = true;
                            break 'walk;
                        }
                    }
                }
            }
        }
    }

    json!({ "success": true, "results": results, "truncated": truncated })
}

/// `FileService.get_disk_usage` — via sysinfo mount table (longest-prefix match).
pub fn disk_usage(path: &str) -> Value {
    if !Path::new(path).exists() {
        return err("Path not found");
    }
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let real = realpath(path);
    let real = real.to_string_lossy();

    let best = disks
        .iter()
        .filter(|d| real.starts_with(&*d.mount_point().to_string_lossy()))
        .max_by_key(|d| d.mount_point().to_string_lossy().len());

    match best {
        Some(d) => {
            let total = d.total_space();
            let free = d.available_space();
            let used = total.saturating_sub(free);
            json!({
                "success": true,
                "path": path,
                "total": total,
                "used": used,
                "free": free,
                "percent": if total > 0 { ((used as f64 / total as f64) * 1000.0).round() / 10.0 } else { 0.0 },
                "total_human": format_size(total),
                "used_human": format_size(used),
                "free_human": format_size(free),
            })
        }
        None => err("Could not determine disk usage"),
    }
}

const SKIP_MOUNT_PREFIXES: &[&str] = &["/snap/", "/var/lib/docker/", "/run/"];
const VIRTUAL_FSTYPES: &[&str] = &[
    "squashfs",
    "tmpfs",
    "devtmpfs",
    "devfs",
    "overlay",
    "aufs",
    "proc",
    "sysfs",
    "cgroup",
    "cgroup2",
    "debugfs",
    "tracefs",
    "securityfs",
    "pstore",
    "efivarfs",
    "bpf",
    "fusectl",
    "configfs",
    "hugetlbfs",
    "mqueue",
    "ramfs",
    "nsfs",
];

/// `FileService.get_all_disk_mounts` — physical mounts, deduped by device.
pub fn disk_mounts() -> Value {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut seen = std::collections::HashSet::new();
    let mut mounts = Vec::new();

    for d in disks.iter() {
        let fstype = d.file_system().to_string_lossy().into_owned();
        let mountpoint = d.mount_point().to_string_lossy().into_owned();
        let device = d.name().to_string_lossy().into_owned();
        if VIRTUAL_FSTYPES.contains(&fstype.as_str()) {
            continue;
        }
        if SKIP_MOUNT_PREFIXES
            .iter()
            .any(|p| mountpoint.starts_with(p))
        {
            continue;
        }
        if !seen.insert(device.clone()) {
            continue;
        }
        let total = d.total_space();
        let free = d.available_space();
        let used = total.saturating_sub(free);
        mounts.push(json!({
            "device": device,
            "mountpoint": mountpoint,
            "fstype": fstype,
            "total": total,
            "used": used,
            "free": free,
            "percent": if total > 0 { ((used as f64 / total as f64) * 1000.0).round() / 10.0 } else { 0.0 },
            "total_human": format_size(total),
            "used_human": format_size(used),
            "free_human": format_size(free),
        }));
    }
    json!({ "success": true, "mounts": mounts })
}

fn dir_size_recursive(path: &Path, max_depth: u32, depth: u32) -> u64 {
    if depth > max_depth {
        return 0;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| {
            let Ok(ft) = e.file_type() else { return 0 };
            if ft.is_file() {
                e.metadata().map(|m| m.len()).unwrap_or(0)
            } else if ft.is_dir() {
                dir_size_recursive(&e.path(), max_depth, depth + 1)
            } else {
                0
            }
        })
        .sum()
}

fn classify_ext(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "ico" => "images",
        "mp4" | "mov" | "avi" | "mkv" | "webm" => "videos",
        "mp3" | "wav" | "ogg" | "flac" => "audio",
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" => "archives",
        "js" | "jsx" | "ts" | "tsx" | "py" | "rs" | "go" | "php" | "rb" | "java" | "c" | "cpp"
        | "h" | "sh" | "css" | "html" | "sql" => "code",
        "txt" | "md" | "pdf" | "doc" | "docx" | "xls" | "xlsx" | "csv" | "json" | "xml" | "yml"
        | "yaml" | "toml" | "ini" | "conf" => "documents",
        _ => "other",
    }
}

fn walk_type_breakdown(
    path: &Path,
    max_depth: u32,
    depth: u32,
    counts: &mut std::collections::BTreeMap<String, (u64, u64)>,
) {
    if depth > max_depth {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            walk_type_breakdown(&entry.path(), max_depth, depth + 1, counts);
        } else if ft.is_file() {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let key = classify_ext(&entry.path()).to_string();
            let bucket = counts.entry(key).or_insert((0, 0));
            bucket.0 += 1;
            bucket.1 += size;
        }
    }
}

pub fn type_breakdown(path: &str, max_depth: u32) -> Value {
    if !is_path_allowed(path) {
        return err(DENIED);
    }
    let p = Path::new(path);
    if !p.exists() {
        return err("Path not found");
    }
    if !p.is_dir() {
        return err("Path is not a directory");
    }
    let mut counts = std::collections::BTreeMap::new();
    walk_type_breakdown(p, max_depth, 0, &mut counts);
    let total_size: u64 = counts.values().map(|(_, size)| *size).sum();
    let total_count: u64 = counts.values().map(|(count, _)| *count).sum();
    let mut breakdown: Vec<Value> = counts
        .into_iter()
        .map(|(category, (count, size))| {
            json!({
                "category": category,
                "type": category,
                "count": count,
                "size": size,
                "size_human": format_size(size),
                "percent": if total_size > 0 { ((size as f64 / total_size as f64) * 1000.0).round() / 10.0 } else { 0.0 },
            })
        })
        .collect();
    breakdown
        .sort_by_key(|v| std::cmp::Reverse(v.get("size").and_then(Value::as_u64).unwrap_or(0)));
    json!({"success":true,"path":path,"max_depth":max_depth,"total_count":total_count,"total_size":total_size,"total_size_human":format_size(total_size),"breakdown":breakdown,"types":breakdown})
}

fn object_root() -> Option<PathBuf> {
    std::env::var("SK_OBJECT_STORAGE_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
}

pub fn object_storage_status() -> Value {
    match object_root() {
        Some(root) => {
            json!({"configured":true,"provider":"local-object","root":root.to_string_lossy()})
        }
        None => {
            json!({"configured":false,"provider":Value::Null,"message":"Object storage is not configured. Set SK_OBJECT_STORAGE_DIR to enable the /files/s3 local-object adapter."})
        }
    }
}

fn object_key_path(path: &str) -> Result<PathBuf, String> {
    let root = object_root().ok_or_else(|| "Object storage is not configured".to_string())?;
    let trimmed = path.trim_start_matches('/');
    let mut out = root.clone();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return Err("Invalid object path".to_string()),
        }
    }
    Ok(out)
}

pub fn object_browse(path: &str) -> Value {
    let Ok(dir) = object_key_path(path) else {
        return err("Object storage is not configured");
    };
    if !dir.exists() {
        return json!({"success":true,"configured":true,"provider":"local-object","path":path,"files":[],"objects":[]});
    }
    if !dir.is_dir() {
        return err("Object path is not a directory");
    }
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return err("Permission denied");
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let name = entry.file_name().to_string_lossy().to_string();
        let key = format!("{}/{}", path.trim_end_matches('/'), name).replace("//", "/");
        files.push(json!({
            "name": name,
            "path": if key.starts_with('/') { key.clone() } else { format!("/{key}") },
            "key": if key.starts_with('/') { key.trim_start_matches('/').to_string() } else { key.clone() },
            "is_dir": meta.is_dir(),
            "size": meta.len(),
            "size_human": format_size(meta.len()),
            "modified": meta.modified().ok().map(isoformat),
        }));
    }
    json!({"success":true,"configured":true,"provider":"local-object","path":path,"files":files,"objects":files})
}

pub fn object_read(path: &str) -> Value {
    let Ok(file) = object_key_path(path) else {
        return err("Object storage is not configured");
    };
    if !file.exists() {
        return err("Object not found");
    }
    if file.is_dir() {
        return err("Object path is a directory");
    }
    match fs::read_to_string(&file) {
        Ok(content) => {
            json!({"success":true,"configured":true,"provider":"local-object","path":path,"content":content})
        }
        Err(e) => err(e.to_string()),
    }
}

pub fn object_write(path: &str, content: &str) -> Value {
    let Ok(file) = object_key_path(path) else {
        return err("Object storage is not configured");
    };
    if let Some(parent) = file.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return err(e.to_string());
        }
    }
    match fs::write(&file, content) {
        Ok(_) => {
            json!({"success":true,"configured":true,"provider":"local-object","path":path,"size":content.len()})
        }
        Err(e) => err(e.to_string()),
    }
}

pub fn object_delete(path: &str) -> Value {
    let Ok(file) = object_key_path(path) else {
        return err("Object storage is not configured");
    };
    let result = if file.is_dir() {
        fs::remove_dir_all(&file)
    } else {
        fs::remove_file(&file)
    };
    match result {
        Ok(_) => json!({"success":true,"configured":true,"provider":"local-object","path":path}),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => err("Object not found"),
        Err(e) => err(e.to_string()),
    }
}

pub fn object_write_bytes(path: &str, data: &[u8]) -> Value {
    let Ok(file) = object_key_path(path) else {
        return err("Object storage is not configured");
    };
    if let Some(parent) = file.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return err(e.to_string());
        }
    }
    match fs::write(&file, data) {
        Ok(_) => {
            json!({"success":true,"configured":true,"provider":"local-object","path":path,"size":data.len()})
        }
        Err(e) => err(e.to_string()),
    }
}

pub fn object_file_path(path: &str) -> Result<PathBuf, String> {
    object_key_path(path)
}

/// `FileService.analyze_directory_sizes`
pub fn analyze(path: &str, depth: u32, limit: usize) -> Value {
    if !is_path_allowed(path) {
        return err(DENIED);
    }
    let p = Path::new(path);
    if !p.exists() {
        return err("Path not found");
    }
    if !p.is_dir() {
        return err("Path is not a directory");
    }

    let Ok(scanner) = fs::read_dir(p) else {
        return err("Permission denied");
    };
    let mut entries: Vec<(u64, bool, Value)> = Vec::new();
    let mut total_size: u64 = 0;

    for entry in scanner.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        let (size, is_dir) = if ft.is_dir() {
            (dir_size_recursive(&entry.path(), depth, 0), true)
        } else if ft.is_file() {
            (entry.metadata().map(|m| m.len()).unwrap_or(0), false)
        } else {
            continue;
        };
        total_size += size;
        entries.push((
            size,
            is_dir,
            json!({
                "name": entry.file_name().to_string_lossy(),
                "path": entry.path().to_string_lossy(),
                "size": size,
                "size_human": format_size(size),
                "is_dir": is_dir,
            }),
        ));
    }

    entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    let with_pct = |mut v: Value, size: u64| {
        v["percent"] = json!(if total_size > 0 {
            ((size as f64 / total_size as f64) * 1000.0).round() / 10.0
        } else {
            0.0
        });
        v
    };

    let directories: Vec<Value> = entries
        .iter()
        .filter(|(_, d, _)| *d)
        .take(limit)
        .map(|(s, _, v)| with_pct(v.clone(), *s))
        .collect();
    let largest_files: Vec<Value> = entries
        .iter()
        .filter(|(_, d, _)| !d)
        .take(limit)
        .map(|(s, _, v)| with_pct(v.clone(), *s))
        .collect();

    json!({
        "success": true,
        "path": path,
        "total_size": total_size,
        "total_size_human": format_size(total_size),
        "directories": directories,
        "largest_files": largest_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist() {
        assert!(!is_path_allowed("/etc/passwd"));
        assert!(!is_path_allowed("/home/../etc/passwd"));
        assert!(is_path_allowed("/var/www/html"));
    }

    #[test]
    fn filemode_string() {
        let meta = fs::metadata("/tmp").unwrap();
        assert!(filemode(&meta).starts_with('d'));
    }
}
