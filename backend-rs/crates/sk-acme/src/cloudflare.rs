//! Cloudflare DNS API — just enough to solve ACME DNS-01: find the zone for
//! a domain and create/delete `_acme-challenge` TXT records.
//!
//! Token comes from `SK_CF_API_TOKEN` (a scoped token with Zone:DNS:Edit).

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

const API: &str = "https://api.cloudflare.com/client/v4";

pub struct Cloudflare {
    token: String,
    http: reqwest::Client,
}

impl Cloudflare {
    pub fn from_env() -> Result<Self> {
        let token = std::env::var("SK_CF_API_TOKEN")
            .context("SK_CF_API_TOKEN not set (Cloudflare token with Zone:DNS:Edit)")?;
        Ok(Self {
            token,
            http: reqwest::Client::new(),
        })
    }

    async fn get(&self, url: &str) -> Result<Value> {
        let v: Value = self
            .http
            .get(url)
            .bearer_auth(&self.token)
            .send()
            .await?
            .json()
            .await?;
        if v["success"].as_bool() != Some(true) {
            return Err(anyhow!("cloudflare GET failed: {}", v["errors"]));
        }
        Ok(v)
    }

    /// Longest-suffix zone match, so `a.b.hyper-mind.dev` resolves to the
    /// `hyper-mind.dev` zone.
    pub async fn zone_id(&self, domain: &str) -> Result<String> {
        let v = self.get(&format!("{API}/zones?per_page=50")).await?;
        let mut best: Option<(usize, String)> = None;
        for z in v["result"].as_array().into_iter().flatten() {
            let name = z["name"].as_str().unwrap_or("");
            if (domain == name || domain.ends_with(&format!(".{name}")))
                && best.as_ref().map(|(l, _)| name.len() > *l).unwrap_or(true)
            {
                best = Some((name.len(), z["id"].as_str().unwrap_or("").to_string()));
            }
        }
        best.map(|(_, id)| id)
            .ok_or_else(|| anyhow!("no Cloudflare zone found for {domain}"))
    }

    /// Delete any existing TXT records with this exact name (clean slate).
    pub async fn delete_txt(&self, zone_id: &str, name: &str) -> Result<()> {
        let v = self
            .get(&format!(
                "{API}/zones/{zone_id}/dns_records?type=TXT&name={name}"
            ))
            .await?;
        for rec in v["result"].as_array().into_iter().flatten() {
            if let Some(id) = rec["id"].as_str() {
                let _ = self
                    .http
                    .delete(format!("{API}/zones/{zone_id}/dns_records/{id}"))
                    .bearer_auth(&self.token)
                    .send()
                    .await;
            }
        }
        Ok(())
    }

    /// Create a TXT record, returning its id (for later cleanup).
    pub async fn create_txt(&self, zone_id: &str, name: &str, content: &str) -> Result<String> {
        let v: Value = self
            .http
            .post(format!("{API}/zones/{zone_id}/dns_records"))
            .bearer_auth(&self.token)
            .json(&serde_json::json!({
                "type": "TXT", "name": name, "content": content, "ttl": 120
            }))
            .send()
            .await?
            .json()
            .await?;
        if v["success"].as_bool() != Some(true) {
            return Err(anyhow!("cloudflare TXT create failed: {}", v["errors"]));
        }
        Ok(v["result"]["id"].as_str().unwrap_or_default().to_string())
    }

    pub async fn delete_record(&self, zone_id: &str, record_id: &str) {
        let _ = self
            .http
            .delete(format!("{API}/zones/{zone_id}/dns_records/{record_id}"))
            .bearer_auth(&self.token)
            .send()
            .await;
    }
}

/// Poll public DNS (Cloudflare DoH) until the TXT value is visible, so we hand
/// off to the ACME CA only after the record has propagated.
pub async fn wait_txt(name: &str, expected: &str, tries: u32) -> bool {
    let http = reqwest::Client::new();
    for _ in 0..tries {
        if let Ok(resp) = http
            .get("https://cloudflare-dns.com/dns-query")
            .query(&[("name", name), ("type", "TXT")])
            .header("accept", "application/dns-json")
            .send()
            .await
        {
            if let Ok(v) = resp.json::<Value>().await {
                let found = v["Answer"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|a| a["data"].as_str().unwrap_or("").trim_matches('"') == expected);
                if found {
                    return true;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
    false
}
