//! sk-acme — Let's Encrypt certificate issuance (pure Rust, no certbot).
//!
//! Two challenge solvers:
//! - **HTTP-01 (default):** writes `<webroot>/.well-known/acme-challenge/<token>`;
//!   nginx serves it (needs the domain reachable on :80, e.g. via a
//!   Cloudflare tunnel).
//! - **DNS-01 (Cloudflare):** creates `_acme-challenge.<domain>` TXT records
//!   via the CF API (needs `SK_CF_API_TOKEN`); works without public :80.
//!
//! Directory: LE production by default, staging when `staging = true`
//! (or `SK_LE_STAGING=1`).

pub mod cloudflare;

use anyhow::{anyhow, Result};
use instant_acme::{
    Account, AuthorizationStatus, ChallengeType, Identifier, LetsEncrypt, NewAccount, NewOrder,
    OrderStatus, RetryPolicy,
};

/// Which ACME challenge to solve.
pub enum Challenge {
    /// Serve the token file from this webroot (`.well-known/acme-challenge/`).
    Http01 { webroot: String },
    /// Cloudflare DNS-01 (`SK_CF_API_TOKEN`).
    Dns01,
}

pub struct Issued {
    pub cert_pem: String,
    pub key_pem: String,
}

/// reqwest and instant-acme both pull rustls; with both aws-lc-rs and ring
/// present via feature unification, rustls can't pick a default — install one.
fn ensure_crypto_provider() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    });
}

fn staging_from_env(explicit: bool) -> bool {
    explicit || std::env::var("SK_LE_STAGING").ok().as_deref() == Some("1")
}

/// The ACME token is the segment of the key authorization before the first
/// dot (`<token>.<thumbprint>`) — used as the HTTP-01 filename.
fn token_of(key_auth: &str) -> &str {
    key_auth.split('.').next().unwrap_or(key_auth)
}

/// Issue a certificate for `domains` (first = CN, all = SANs).
pub async fn issue(
    domains: &[String],
    email: &str,
    challenge: Challenge,
    staging: bool,
) -> Result<Issued> {
    if domains.is_empty() {
        return Err(anyhow!("no domains provided"));
    }
    ensure_crypto_provider();
    let staging = staging_from_env(staging);
    let directory = if staging {
        LetsEncrypt::Staging.url()
    } else {
        LetsEncrypt::Production.url()
    };
    tracing::info!(?domains, staging, "requesting Let's Encrypt certificate");

    let contact = format!("mailto:{email}");
    let (account, _creds) = Account::builder()?
        .create(
            &NewAccount {
                contact: &[contact.as_str()],
                terms_of_service_agreed: true,
                only_return_existing: false,
            },
            directory.to_string(),
            None,
        )
        .await?;

    let identifiers: Vec<Identifier> = domains.iter().map(|d| Identifier::Dns(d.clone())).collect();
    let mut order = account.new_order(&NewOrder::new(&identifiers)).await?;

    // Cleanup handles per challenge type.
    let mut http_files: Vec<std::path::PathBuf> = Vec::new();
    let mut dns_records: Vec<(String, String)> = Vec::new(); // (zone_id, record_id)
    let cf = match &challenge {
        Challenge::Dns01 => Some(cloudflare::Cloudflare::from_env()?),
        Challenge::Http01 { .. } => None,
    };

    // Provision each authorization's challenge response.
    {
        let mut authorizations = order.authorizations();
        while let Some(result) = authorizations.next().await {
            let mut authz = result?;
            match authz.status {
                AuthorizationStatus::Pending => {}
                AuthorizationStatus::Valid => continue,
                other => return Err(anyhow!("unexpected authorization status: {other:?}")),
            }

            match &challenge {
                Challenge::Http01 { webroot } => {
                    let mut ch = authz
                        .challenge(ChallengeType::Http01)
                        .ok_or_else(|| anyhow!("no http-01 challenge offered"))?;
                    let key_auth = ch.key_authorization();
                    let value = key_auth.as_str().to_string();
                    let token = token_of(&value).to_string();

                    let dir = std::path::Path::new(webroot).join(".well-known/acme-challenge");
                    std::fs::create_dir_all(&dir)?;
                    let file = dir.join(&token);
                    std::fs::write(&file, &value)?;
                    // world-readable so nginx (www-data) can serve it
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let _ =
                            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644));
                    }
                    http_files.push(file);
                    ch.set_ready().await?;
                }
                Challenge::Dns01 => {
                    let cf = cf.as_ref().unwrap();
                    let mut ch = authz
                        .challenge(ChallengeType::Dns01)
                        .ok_or_else(|| anyhow!("no dns-01 challenge offered"))?;
                    let domain = ch.identifier().to_string();
                    let value = ch.key_authorization().dns_value();
                    let record = format!("_acme-challenge.{domain}");
                    let zone_id = cf.zone_id(&domain).await?;
                    cf.delete_txt(&zone_id, &record).await?;
                    let rid = cf.create_txt(&zone_id, &record, &value).await?;
                    dns_records.push((zone_id, rid));
                    // wait for propagation before telling the CA to validate
                    if !cloudflare::wait_txt(&record, &value, 40).await {
                        tracing::warn!(%record, "TXT not observed via DoH; proceeding anyway");
                    }
                    ch.set_ready().await?;
                }
            }
        }
    }

    // Drive to completion, always cleaning up challenge artifacts.
    let result = finish(&mut order).await;

    for f in &http_files {
        let _ = std::fs::remove_file(f);
    }
    if let Some(cf) = &cf {
        for (zone, rid) in &dns_records {
            cf.delete_record(zone, rid).await;
        }
    }

    result
}

async fn finish(order: &mut instant_acme::Order) -> Result<Issued> {
    let status = order.poll_ready(&RetryPolicy::default()).await?;
    if status != OrderStatus::Ready {
        return Err(anyhow!("order not ready: {status:?}"));
    }
    let key_pem = order.finalize().await?;
    let cert_pem = order.poll_certificate(&RetryPolicy::default()).await?;
    Ok(Issued { cert_pem, key_pem })
}
