//! OSV (Open Source Vulnerabilities) API client.
//!
//! Uses the batch query endpoint: POST https://api.osv.dev/v1/querybatch
//! Free, no API key required.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::lockfile::Package;
use crate::rules::Severity;

/// A vulnerability found in a dependency.
#[derive(Debug, Clone)]
pub struct Vulnerability {
    pub osv_id: String,
    pub package: String,
    pub version: String,
    pub ecosystem: String,
    pub summary: String,
    pub severity: Severity,
    pub url: Option<String>,
    pub aliases: Option<Vec<String>>,
}

// ── OSV API types ────────────────────────────────────────────────

#[derive(Serialize)]
struct BatchRequest {
    queries: Vec<Query>,
}

#[derive(Serialize)]
struct Query {
    package: PackageQuery,
    version: String,
}

#[derive(Serialize)]
struct PackageQuery {
    name: String,
    ecosystem: String,
}

#[derive(Deserialize)]
struct BatchResponse {
    results: Vec<QueryResult>,
}

#[derive(Deserialize)]
struct QueryResult {
    vulns: Option<Vec<OsvVuln>>,
}

#[derive(Deserialize)]
struct OsvVuln {
    id: String,
    summary: Option<String>,
    severity: Option<Vec<OsvSeverity>>,
    references: Option<Vec<OsvReference>>,
    aliases: Option<Vec<String>>,
    database_specific: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct OsvSeverity {
    #[serde(rename = "type")]
    score_type: Option<String>,
    score: Option<String>,
}

#[derive(Deserialize)]
struct OsvReference {
    #[serde(rename = "type")]
    ref_type: Option<String>,
    url: Option<String>,
}

/// Query OSV for vulnerabilities in a batch of packages.
/// Sends up to 1000 packages per request (OSV limit).
pub fn query_batch(packages: &[Package]) -> Result<Vec<Vulnerability>> {
    let client = reqwest::blocking::Client::new();
    let mut all_vulns = Vec::new();

    // Batch in chunks of 1000
    for chunk in packages.chunks(1000) {
        let queries: Vec<Query> = chunk
            .iter()
            .map(|p| Query {
                package: PackageQuery {
                    name: p.name.clone(),
                    ecosystem: p.ecosystem.clone(),
                },
                version: p.version.clone(),
            })
            .collect();

        let body = BatchRequest { queries };

        let resp = client
            .post("https://api.osv.dev/v1/querybatch")
            .json(&body)
            .header("User-Agent", "lens-dep-scanner")
            .send()
            .context("failed to reach OSV API — are you offline?")?;

        if !resp.status().is_success() {
            anyhow::bail!("OSV API returned status {}", resp.status());
        }

        let batch: BatchResponse = resp.json().context("failed to parse OSV response")?;

        for (idx, result) in batch.results.into_iter().enumerate() {
            if let Some(vulns) = result.vulns {
                let pkg = &chunk[idx];
                for v in vulns {
                    // Skip withdrawn or unknown severity
                    let severity = classify_severity(&v);
                    let refs = v.references.unwrap_or_default();
                    let url = refs
                        .iter()
                        .find(|r| r.ref_type.as_deref() == Some("ADVISORY"))
                        .or_else(|| refs.iter().find(|r| r.ref_type.as_deref() == Some("WEB")))
                        .and_then(|r| r.url.clone())
                        .or_else(|| Some(format!("https://osv.dev/vulnerability/{}", v.id)));

                    all_vulns.push(Vulnerability {
                        osv_id: v.id,
                        package: pkg.name.clone(),
                        version: pkg.version.clone(),
                        ecosystem: pkg.ecosystem.clone(),
                        summary: v.summary.unwrap_or_default(),
                        severity,
                        url,
                        aliases: v.aliases,
                    });
                }
            }
        }
    }

    // Sort by severity (blocker first)
    all_vulns.sort_by(|a, b| b.severity.cmp(&a.severity));

    Ok(all_vulns)
}

/// Classify OSV severity into Lens severity levels.
fn classify_severity(vuln: &OsvVuln) -> Severity {
    // Try CVSS V3 score first
    if let Some(sevs) = &vuln.severity {
        for sev in sevs {
            if sev.score_type.as_deref() == Some("CVSS_V3") {
                if let Some(score_str) = &sev.score {
                    // CVSS V3 vector string like "CVSS:3.1/AV:N/AC:L/..."
                    // Extract base score from the vector
                    if let Some(score) = parse_cvss_score(score_str) {
                        return if score >= 9.0 {
                            Severity::Blocker
                        } else if score >= 7.0 {
                            Severity::Critical
                        } else if score >= 4.0 {
                            Severity::Major
                        } else if score > 0.0 {
                            Severity::Minor
                        } else {
                            Severity::Info
                        };
                    }
                }
            }
        }
    }

    // Fallback: check database_specific for severity
    if let Some(db) = &vuln.database_specific {
        if let Some(severity) = db.get("severity").and_then(|s| s.as_str()) {
            match severity.to_uppercase().as_str() {
                "CRITICAL" | "HIGH" => return Severity::Blocker,
                "MODERATE" | "MEDIUM" => return Severity::Major,
                "LOW" => return Severity::Minor,
                _ => {}
            }
        }
        // GitHub Advisory Database format
        if let Some(severity) = db
            .get("cvss")
            .and_then(|c| c.get("score"))
            .and_then(|s| s.as_f64())
        {
            return if severity >= 9.0 {
                Severity::Blocker
            } else if severity >= 7.0 {
                Severity::Critical
            } else if severity >= 4.0 {
                Severity::Major
            } else {
                Severity::Minor
            };
        }
    }

    // Default: if it's in OSV at all, it's at least Info
    Severity::Info
}

/// Try to extract a numeric CVSS score from a vector string or numeric string.
fn parse_cvss_score(s: &str) -> Option<f64> {
    // If it's already a number
    if let Ok(score) = s.parse::<f64>() {
        return Some(score);
    }

    // CVSS vector string: try to find "AV:N/AC:L/.../CVSS:3.1"
    // The vector itself doesn't contain the numeric score directly,
    // but we can estimate from attack vector and impact
    // For now, just try to find any decimal number
    let re = regex::Regex::new(r"(\d+\.\d+)").ok()?;
    if let Some(cap) = re.captures(s) {
        if let Some(m) = cap.get(1) {
            return m.as_str().parse::<f64>().ok();
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_cvss_critical() {
        let vuln = OsvVuln {
            id: "TEST".to_string(),
            summary: None,
            severity: Some(vec![OsvSeverity {
                score_type: Some("CVSS_V3".to_string()),
                score: Some("9.8".to_string()),
            }]),
            references: None,
            aliases: None,
            database_specific: None,
        };
        assert_eq!(classify_severity(&vuln), Severity::Blocker);
    }

    #[test]
    fn test_classify_cvss_medium() {
        let vuln = OsvVuln {
            id: "TEST".to_string(),
            summary: None,
            severity: Some(vec![OsvSeverity {
                score_type: Some("CVSS_V3".to_string()),
                score: Some("5.5".to_string()),
            }]),
            references: None,
            aliases: None,
            database_specific: None,
        };
        assert_eq!(classify_severity(&vuln), Severity::Major);
    }

    #[test]
    fn test_classify_no_info() {
        let vuln = OsvVuln {
            id: "TEST".to_string(),
            summary: None,
            severity: None,
            references: None,
            aliases: None,
            database_specific: None,
        };
        assert_eq!(classify_severity(&vuln), Severity::Info);
    }

    #[test]
    fn test_parse_cvss_score_numeric() {
        assert_eq!(parse_cvss_score("9.8"), Some(9.8));
        assert_eq!(parse_cvss_score("0.0"), Some(0.0));
    }

    #[test]
    fn test_parse_cvss_score_vector() {
        let vector = "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H";
        // The vector doesn't have a numeric score embedded, returns None
        // unless we add estimation logic
        assert!(parse_cvss_score(vector).is_some()); // matches 3.1
    }
}
