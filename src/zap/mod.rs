//! OWASP ZAP integration — Dynamic Application Security Testing (DAST).
//!
//! Uses ZAP's REST API to spider and actively scan a running web application.
//! Two modes:
//!   1. Docker mode: auto-start `zaproxy/zap-stable` container
//!   2. Connect mode: connect to an existing ZAP instance via `--zap-host`

pub mod client;
pub mod report;

use anyhow::{bail, Context};
use owo_colors::OwoColorize;

use crate::cli::ZapArgs;

/// Run `lens zap` — scan a running web application with OWASP ZAP.
pub fn run_zap(args: ZapArgs) -> anyhow::Result<std::process::ExitCode> {
    let target = &args.target;

    // Validate URL
    if !target.starts_with("http://") && !target.starts_with("https://") {
        bail!(
            "target must be a full URL (http:// or https://), got: {}",
            target
        );
    }

    let is_json = matches!(args.format, crate::cli::Format::Json);
    let zap_host = args
        .zap_host
        .clone()
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string());
    let zap_api_key = args.zap_key.clone().unwrap_or_default();

    let mut container_id: Option<String> = None;

    // Start ZAP via Docker if needed
    let effective_host = if args.no_docker {
        if !is_json {
            println!("  {} Connecting to ZAP at {}", "\u{1F517}", zap_host.bold());
        }
        zap_host.clone()
    } else {
        // Try to start ZAP container
        if !is_json {
            println!("  {} Starting OWASP ZAP container...", "\u{1F680}");
        }
        match start_zap_container(&args) {
            Ok(host) => {
                container_id = Some(host.1);
                if !is_json {
                    println!(
                        "  {} ZAP container started ({})",
                        "\u{2705}".green(),
                        host.0.bold()
                    );
                }
                host.0
            }
            Err(e) => {
                // Fallback: try connecting to existing ZAP
                if !is_json {
                    println!(
                        "  {} Could not start Docker: {}",
                        "\u{26A0}\u{FE0F}".yellow(),
                        e.to_string().dimmed()
                    );
                    println!(
                        "  {} Falling back to existing ZAP at {}",
                        "\u{1F517}",
                        zap_host.bold()
                    );
                }
                zap_host.clone()
            }
        }
    };

    // Wait for ZAP to be ready
    if !is_json {
        println!("  {} Waiting for ZAP API to be ready...", "\u{23F3}");
    }
    let client = client::ZapClient::new(&effective_host, &zap_api_key);
    client
        .wait_for_ready()
        .context("ZAP API not responding — is ZAP running?")?;

    if !is_json {
        println!("  {} ZAP is ready!", "\u{2705}".green());
    }

    // Spider the target
    if !is_json {
        println!("  {} Spidering {}...", "\u{1F577}\u{FE0F}", target.bold());
    }
    let spider_count = client.spider(target, &args)?;
    if !is_json {
        println!("  {} Spidered {} URLs", "\u{2705}".green(), spider_count);
    }

    // Optional: AJAX spider for SPAs
    if args.ajax {
        if !is_json {
            println!(
                "  {} AJAX spidering {}...",
                "\u{1F578}\u{FE0F}",
                target.bold()
            );
        }
        client.ajax_spider(target)?;
        if !is_json {
            println!("  {} AJAX spider complete", "\u{2705}".green());
        }
    }

    // Active scan
    if !is_json {
        println!(
            "  {} Running active scan on {}...",
            "\u{1F50D}".bold(),
            target.bold()
        );
    }
    client.active_scan(target, &args)?;
    if !is_json {
        println!("  {} Active scan complete!", "\u{2705}".green());
    }

    // Fetch alerts
    let alerts = client.alerts(target)?;

    // Stop/cleanup container
    if let Some(cid) = &container_id {
        let _ = stop_container(cid);
    }

    // Report
    if alerts.is_empty() {
        if is_json {
            print_json_report(target, &alerts, 0, 0, 0, 0, 0);
        } else {
            println!("  {} No vulnerabilities found!", "\u{2705}".green());
        }
        return Ok(std::process::ExitCode::SUCCESS);
    }

    // Count severities
    let mut blocker = 0u32;
    let mut critical = 0u32;
    let mut major = 0u32;
    let mut minor = 0u32;
    let mut info = 0u32;

    for a in &alerts {
        match a.risk {
            3 => blocker += 1,
            2 => critical += 1,
            1 => major += 1,
            0 => info += 1,
            _ => minor += 1,
        }
    }

    if is_json {
        print_json_report(target, &alerts, blocker, critical, major, minor, info);
        if args.gate && (blocker > 0 || critical > 0) {
            return Ok(std::process::ExitCode::from(1));
        }
        return Ok(std::process::ExitCode::SUCCESS);
    }

    // Terminal report
    println!();
    println!("  {} ZAP Scan Results", "\u{1F6E1}\u{FE0F}".bold());
    println!("  {}", "\u{2500}".repeat(60).dimmed());

    for a in &alerts {
        let (emoji, label) = match a.risk {
            3 => ("\u{1F534}", "HIGH"),
            2 => ("\u{1F7E0}", "MEDIUM"),
            1 => ("\u{1F7E1}", "LOW"),
            0 => ("\u{26AA}", "INFO"),
            _ => ("\u{1F535}", "OTHER"),
        };
        println!(
            "  {} {} {} [{}]",
            emoji,
            label.bold(),
            a.alert.bold(),
            format!("CWE-{}", a.cwe_id).dimmed()
        );
        if !a.description.is_empty() {
            println!(
                "    {}",
                a.description
                    .dimmed()
                    .to_string()
                    .chars()
                    .take(100)
                    .collect::<String>()
            );
        }
        for inst in &a.instances {
            println!(
                "    {} {} {}",
                "\u{2192}".dimmed(),
                inst.method.dimmed(),
                inst.uri.cyan()
            );
        }
        if !a.solution.is_empty() {
            let sol: String = a.solution.chars().take(120).collect();
            println!("    {} {}", "Fix:".dimmed(), sol.dimmed());
        }
        if let Some(ref ref_url) = a.reference {
            println!("    {} {}", "\u{1F517}".dimmed(), ref_url.dimmed());
        }
    }

    println!();
    println!("  {} Summary", "\u{1F4CA}".bold());
    println!("  {}", "\u{2500}".repeat(60).dimmed());
    println!(
        "  {} Spidered {} URLs, {} alerts found",
        "\u{1F6E1}\u{FE0F}",
        spider_count,
        alerts.len()
    );
    println!(
        "  {} {} high, {} medium, {} low, {} info",
        "\u{26A0}\u{FE0F}",
        blocker.to_string().red().bold(),
        critical.to_string().yellow(),
        major.to_string().blue(),
        info
    );

    if args.gate && (blocker > 0 || critical > 0) {
        println!(
            "  {} Quality gate failed: {} high + {} medium vulnerabilities",
            "\u{274C}".red(),
            blocker,
            critical
        );
        return Ok(std::process::ExitCode::from(1));
    }

    Ok(std::process::ExitCode::SUCCESS)
}

fn print_json_report(
    target: &str,
    alerts: &[client::ZapAlert],
    blocker: u32,
    critical: u32,
    major: u32,
    minor: u32,
    info: u32,
) {
    use serde::Serialize;

    #[derive(Serialize)]
    struct AlertJson {
        alert: String,
        risk: u32,
        risk_label: String,
        cwe_id: i64,
        description: String,
        solution: String,
        reference: Option<String>,
        instances: Vec<InstanceJson>,
    }

    #[derive(Serialize)]
    struct InstanceJson {
        url: String,
        method: String,
        param: String,
    }

    #[derive(Serialize)]
    struct Report {
        target: String,
        scanner: String,
        total_alerts: usize,
        high: u32,
        medium: u32,
        low: u32,
        info: u32,
        alerts: Vec<AlertJson>,
    }

    let alert_json: Vec<AlertJson> = alerts
        .iter()
        .map(|a| AlertJson {
            alert: a.alert.clone(),
            risk: a.risk,
            risk_label: match a.risk {
                3 => "high".to_string(),
                2 => "medium".to_string(),
                1 => "low".to_string(),
                _ => "info".to_string(),
            },
            cwe_id: a.cwe_id,
            description: a.description.clone(),
            solution: a.solution.clone(),
            reference: a.reference.clone(),
            instances: a
                .instances
                .iter()
                .map(|i| InstanceJson {
                    url: i.uri.clone(),
                    method: i.method.clone(),
                    param: i.param.clone(),
                })
                .collect(),
        })
        .collect();

    let report = Report {
        target: target.to_string(),
        scanner: "OWASP ZAP".to_string(),
        total_alerts: alerts.len(),
        high: blocker,
        medium: critical,
        low: major,
        info,
        alerts: alert_json,
    };

    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

/// Start ZAP container via Docker. Returns (host_url, container_id).
fn start_zap_container(args: &ZapArgs) -> anyhow::Result<(String, String)> {
    let port = args.zap_port.unwrap_or(8080);
    let image = &args.zap_image;

    // Check if Docker is available
    let docker_check = std::process::Command::new("docker")
        .arg("--version")
        .output()
        .context("docker not found")?;

    if !docker_check.status.success() {
        bail!("docker command failed");
    }

    // Pull image (silently)
    let _ = std::process::Command::new("docker")
        .args(["pull", image])
        .output();

    // Run ZAP container
    let output = std::process::Command::new("docker")
        .args([
            "run",
            "-d",
            "--rm",
            "-p",
            &format!("{}:8080", port),
            "-e",
            "ZAP_CLI_API_KEY=",
            image,
            "zap.sh",
            "-daemon",
            "-host",
            "0.0.0.0",
            "-port",
            "8080",
            "-config",
            "api.disablekey=true",
            "-config",
            "scanner.attackOnStart=true",
            "-config",
            "view.mode=attack",
        ])
        .output()
        .context("failed to start ZAP container")?;

    if !output.status.success() {
        bail!(
            "docker run failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

    Ok((format!("http://127.0.0.1:{}", port), container_id))
}

fn stop_container(container_id: &str) -> anyhow::Result<()> {
    std::process::Command::new("docker")
        .args(["stop", container_id])
        .output()?;
    Ok(())
}
