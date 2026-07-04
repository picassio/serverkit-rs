//! sk-acme-cli — issue a cert from the shell, for testing the ACME pipeline.
//!
//!   sk-acme-cli --http --webroot /var/www/letsencrypt --email you@x.com \
//!       --out-dir /tmp/cert --staging domain1 [domain2 ...]
//!   sk-acme-cli --dns  --email you@x.com --out-dir /tmp/cert domain1 ...

use sk_acme::Challenge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut method = "http";
    let mut webroot = "/var/www/letsencrypt".to_string();
    let mut email = "admin@example.com".to_string();
    let mut out_dir = "/tmp/sk-cert".to_string();
    let mut staging = false;
    let mut domains = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--http" => method = "http",
            "--dns" => method = "dns",
            "--staging" => staging = true,
            "--webroot" => {
                i += 1;
                webroot = args[i].clone();
            }
            "--email" => {
                i += 1;
                email = args[i].clone();
            }
            "--out-dir" => {
                i += 1;
                out_dir = args[i].clone();
            }
            d => domains.push(d.to_string()),
        }
        i += 1;
    }
    if domains.is_empty() {
        eprintln!("usage: sk-acme-cli [--http|--dns] [--staging] --email E --webroot W --out-dir D domain...");
        std::process::exit(2);
    }

    let challenge = if method == "dns" {
        Challenge::Dns01
    } else {
        Challenge::Http01 { webroot }
    };

    let issued = sk_acme::issue(&domains, &email, challenge, staging).await?;
    std::fs::create_dir_all(&out_dir)?;
    let cert = format!("{out_dir}/fullchain.pem");
    let key = format!("{out_dir}/privkey.pem");
    std::fs::write(&cert, &issued.cert_pem)?;
    std::fs::write(&key, &issued.key_pem)?;
    println!("OK\ncert: {cert}\nkey:  {key}");
    Ok(())
}
