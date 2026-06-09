//! ZAP REST API client.
//!
//! Uses the ZAP API v2 (JSON endpoints) for spidering, scanning, and fetching alerts.

use anyhow::{bail, Context};
use serde::Deserialize;

use crate::cli::ZapArgs;

/// ZAP API client.
pub struct ZapClient {
    base: String,
    api_key: String,
    http: reqwest::blocking::Client,
}

/// A ZAP alert (vulnerability finding).
#[derive(Debug, Clone)]
pub struct ZapAlert {
    pub alert: String,
    pub risk: u32,
    pub cwe_id: i64,
    pub description: String,
    pub solution: String,
    pub reference: Option<String>,
    pub instances: Vec<ZapInstance>,
}

/// A specific instance of an alert (URL + method).
#[derive(Debug, Clone, Deserialize)]
pub struct ZapInstance {
    pub uri: String,
    pub method: String,
    #[serde(default)]
    pub param: String,
}

impl ZapInstance {
    pub fn url(&self) -> &str {
        &self.uri
    }
}

#[derive(Deserialize)]
struct ApiResponse {
    #[serde(default)]
    Result: Option<serde_json::Value>,
    #[serde(default)]
    results: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Deserialize)]
struct AlertResponse {
    alerts: Vec<AlertRaw>,
}

#[derive(Deserialize)]
struct AlertRaw {
    #[serde(default)]
    alert: String,
    #[serde(default, rename = "riskcode")]
    risk: String,
    #[serde(default)]
    cweid: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    solution: String,
    #[serde(default)]
    reference: String,
    #[serde(default)]
    instances: Vec<InstanceRaw>,
}

#[derive(Deserialize)]
struct InstanceRaw {
    #[serde(default)]
    uri: String,
    #[serde(default)]
    method: String,
    #[serde(default)]
    param: String,
}

#[derive(Deserialize)]
struct StatusResponse {
    #[serde(default)]
    status: String,
}

#[derive(Deserialize)]
struct NumericResponse {
    #[serde(default)]
    Result: String,
}

impl ZapClient {
    pub fn new(base: &str, api_key: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            http: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap(),
        }
    }

    fn url(&self, path: &str) -> String {
        let sep = if path.starts_with('?') { "" } else { "/" };
        format!("{}{}{}{}", self.base, "/JSON", sep, path)
    }

    fn get(&self, path: &str) -> anyhow::Result<serde_json::Value> {
        let mut url = self.url(path);
        if !self.api_key.is_empty() {
            url = format!("{}&apikey={}", url, self.api_key);
        }
        let resp = self
            .http
            .get(&url)
            .send()
            .context("ZAP API request failed")?;
        if !resp.status().is_success() {
            bail!("ZAP API returned status {}", resp.status());
        }
        let val: serde_json::Value = resp.json().context("parsing ZAP response")?;
        // Check for ZAP error
        if let Some(code) = val.get("code").and_then(|c| c.as_str()) {
            if code != "200" && code != "ok" {
                let msg = val
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown");
                bail!("ZAP error {}: {}", code, msg);
            }
        }
        Ok(val)
    }

    /// Wait for ZAP API to be ready (poll up to 120s).
    pub fn wait_for_ready(&self) -> anyhow::Result<()> {
        let max_wait = std::time::Duration::from_secs(120);
        let start = std::time::Instant::now();

        while start.elapsed() < max_wait {
            if let Ok(val) = self.get("core/view/version") {
                if val.get("version").is_some() || val.get("Result").is_some() {
                    return Ok(());
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        bail!("ZAP API not ready after {}s", max_wait.as_secs())
    }

    /// Run spider scan. Returns number of URLs found.
    pub fn spider(&self, target: &str, args: &ZapArgs) -> anyhow::Result<u32> {
        let max_depth = args.max_depth.unwrap_or(5);
        let path = format!(
            "/spider/action/scan/?url={}&maxChildren={}&recurse=true",
            urlencoding::encode(target),
            max_depth
        );
        let val = self.get(&path)?;
        let scan_id = val
            .get("scan")
            .and_then(|s| s.as_str())
            .unwrap_or("0")
            .parse::<u32>()
            .unwrap_or(0);

        // Wait for spider to complete
        let max_wait = std::time::Duration::from_secs(args.timeout as u64);
        let start = std::time::Instant::now();
        while start.elapsed() < max_wait {
            let status = self.get(&format!("/spider/view/status/?scanId={}", scan_id))?;
            let pct = status.get("status").and_then(|s| s.as_str()).unwrap_or("0");
            if pct == "100" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        // Get results count
        let results = self.get(&format!("/spider/view/results/?scanId={}", scan_id))?;
        let urls = results
            .get("results")
            .and_then(|r| r.as_array())
            .map(|a| a.len())
            .unwrap_or(0) as u32;

        Ok(urls)
    }

    /// Run AJAX spider for SPA applications.
    pub fn ajax_spider(&self, target: &str) -> anyhow::Result<()> {
        let path = format!(
            "/spider/action/scanAsUser/?url={}&contextId=1&userId=1",
            urlencoding::encode(target)
        );
        let _ = self.get(&path);

        // Wait for completion (max 60s for AJAX spider)
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(60) {
            let status = self.get("/spider/view/status/?scanId=-1")?;
            let pct = status.get("status").and_then(|s| s.as_str()).unwrap_or("0");
            if pct == "100" || pct == "stopped" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        Ok(())
    }

    /// Run active scan.
    pub fn active_scan(&self, target: &str, args: &ZapArgs) -> anyhow::Result<()> {
        let path = format!(
            "/ascan/action/scan/?url={}&recurse=true",
            urlencoding::encode(target)
        );
        let val = self.get(&path)?;
        let scan_id = val
            .get("scan")
            .and_then(|s| s.as_str())
            .unwrap_or("0")
            .parse::<u32>()
            .unwrap_or(0);

        // Wait for active scan to complete
        let max_wait = std::time::Duration::from_secs(args.timeout as u64 * 2);
        let start = std::time::Instant::now();
        while start.elapsed() < max_wait {
            let status = self.get(&format!("/ascan/view/status/?scanId={}", scan_id))?;
            let pct = status.get("status").and_then(|s| s.as_str()).unwrap_or("0");
            if pct == "100" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        Ok(())
    }

    /// Fetch all alerts for a target.
    pub fn alerts(&self, target: &str) -> anyhow::Result<Vec<ZapAlert>> {
        let base_url = format!(
            "/core/view/alerts/?baseurl={}&start=0&count=1000",
            urlencoding::encode(target)
        );
        let val = self.get(&base_url)?;

        let raw_alerts = val
            .get("alerts")
            .and_then(|a| a.as_array())
            .cloned()
            .unwrap_or_default();

        let mut alerts = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for raw in &raw_alerts {
            let alert_name = raw
                .get("alert")
                .and_then(|a| a.as_str())
                .unwrap_or("")
                .to_string();

            // Deduplicate by alert name
            if !seen.insert(alert_name.clone()) {
                continue;
            }

            let risk = raw
                .get("riskcode")
                .and_then(|r| r.as_str())
                .unwrap_or("0")
                .parse::<u32>()
                .unwrap_or(0);

            let cwe_id = raw
                .get("cweid")
                .and_then(|c| c.as_str())
                .unwrap_or("0")
                .parse::<i64>()
                .unwrap_or(0);

            let description = raw
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();

            let solution = raw
                .get("solution")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();

            let reference = raw
                .get("reference")
                .and_then(|r| r.as_str())
                .map(|s| s.to_string());

            // Collect instances
            let instances: Vec<ZapInstance> = raw
                .get("instances")
                .and_then(|i| i.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|inst| {
                            Some(ZapInstance {
                                uri: inst.get("uri")?.as_str()?.to_string(),
                                method: inst.get("method")?.as_str()?.to_string(),
                                param: inst
                                    .get("param")
                                    .and_then(|p| p.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            alerts.push(ZapAlert {
                alert: alert_name,
                risk,
                cwe_id,
                description,
                solution,
                reference,
                instances,
            });
        }

        // Sort by risk (high first)
        alerts.sort_by(|a, b| b.risk.cmp(&a.risk));

        Ok(alerts)
    }
}
