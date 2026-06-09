//! Lock file parser — extracts package name + version from dependency lock files.
//!
//! Supported formats:
//! - npm: package-lock.json (v2/v3)
//! - Cargo: Cargo.lock
//! - Go: go.sum
//! - Pub: pubspec.lock

use anyhow::Result;
use std::path::{Path, PathBuf};

/// A parsed dependency entry.
#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub ecosystem: String, // "npm", "crates.io", "Go", "Pub"
    pub source_file: PathBuf,
}

/// Discover and parse all recognized lock files under `root`.
pub fn discover_and_parse(root: &Path) -> Result<Vec<Package>> {
    let mut packages = Vec::new();

    // package-lock.json
    if let Ok(pkgs) = parse_npm(root) {
        packages.extend(pkgs);
    }

    // Cargo.lock
    if let Ok(pkgs) = parse_cargo(root) {
        packages.extend(pkgs);
    }

    // go.sum
    if let Ok(pkgs) = parse_gosum(root) {
        packages.extend(pkgs);
    }

    // pubspec.lock
    if let Ok(pkgs) = parse_pubspec(root) {
        packages.extend(pkgs);
    }

    Ok(packages)
}

/// Parse package-lock.json (npm).
fn parse_npm(root: &Path) -> Result<Vec<Package>> {
    let path = root.join("package-lock.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path)?;
    let val: serde_json::Value = serde_json::from_str(&text)?;

    let mut packages = Vec::new();

    // v2/v3 format: .dependencies.{name}.version
    if let Some(deps) = val.get("dependencies").and_then(|d| d.as_object()) {
        for (name, info) in deps {
            if let Some(version) = info.get("version").and_then(|v| v.as_str()) {
                packages.push(Package {
                    name: name.clone(),
                    version: version.to_string(),
                    ecosystem: "npm".to_string(),
                    source_file: path.clone(),
                });
            }
        }
    }

    // v1 format: .packages.{path}.version
    if packages.is_empty() {
        if let Some(pkgs) = val.get("packages").and_then(|p| p.as_object()) {
            for (dir, info) in pkgs {
                // Skip root package (empty string key) and linked packages
                if dir.is_empty() || dir.starts_with("file:") {
                    continue;
                }
                let name = dir.rsplit("node_modules/").next().unwrap_or(dir);
                if let Some(version) = info.get("version").and_then(|v| v.as_str()) {
                    packages.push(Package {
                        name: name.to_string(),
                        version: version.to_string(),
                        ecosystem: "npm".to_string(),
                        source_file: path.clone(),
                    });
                }
            }
        }
    }

    Ok(packages)
}

/// Parse Cargo.lock (Rust/crates.io).
fn parse_cargo(root: &Path) -> Result<Vec<Package>> {
    let path = root.join("Cargo.lock");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path)?;

    let mut packages = Vec::new();
    let mut in_package = false;
    let mut name = String::new();
    let mut version = String::new();
    let mut source = String::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            // Flush previous package
            if !name.is_empty() && !version.is_empty() && source.contains("crates.io") {
                packages.push(Package {
                    name: name.clone(),
                    version: version.clone(),
                    ecosystem: "crates.io".to_string(),
                    source_file: path.clone(),
                });
            }
            in_package = true;
            name.clear();
            version.clear();
            source.clear();
            continue;
        }
        if trimmed.starts_with('[') && !trimmed.starts_with("[[") {
            // New section — flush
            if in_package && !name.is_empty() && !version.is_empty() {
                packages.push(Package {
                    name: name.clone(),
                    version: version.clone(),
                    ecosystem: "crates.io".to_string(),
                    source_file: path.clone(),
                });
            }
            in_package = false;
            continue;
        }
        if in_package {
            if let Some(val) = trimmed.strip_prefix("name = ") {
                name = val.trim_matches('"').to_string();
            } else if let Some(val) = trimmed.strip_prefix("version = ") {
                version = val.trim_matches('"').to_string();
            } else if let Some(val) = trimmed.strip_prefix("source = ") {
                source = val.trim_matches('"').to_string();
            }
        }
    }

    // Flush last package
    if in_package && !name.is_empty() && !version.is_empty() {
        packages.push(Package {
            name: name.clone(),
            version: version.clone(),
            ecosystem: "crates.io".to_string(),
            source_file: path.clone(),
        });
    }

    Ok(packages)
}

/// Parse go.sum (Go modules).
fn parse_gosum(root: &Path) -> Result<Vec<Package>> {
    let path = root.join("go.sum");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path)?;

    let mut packages = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Format: module/path v1.2.3 h1:hash...  or  module/path v1.2.3/go.mod h1:hash...
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        if parts.len() < 2 {
            continue;
        }
        let module = parts[0];
        let version = parts[1];

        // Skip /go.mod entries
        if version.contains("/go.mod") {
            continue;
        }

        // Deduplicate (same module+version appears twice in go.sum)
        let key = format!("{}@{}", module, version);
        if seen.insert(key) {
            packages.push(Package {
                name: module.to_string(),
                version: version.trim_start_matches('v').to_string(),
                ecosystem: "Go".to_string(),
                source_file: path.clone(),
            });
        }
    }

    Ok(packages)
}

/// Parse pubspec.lock (Dart/Flutter Pub).
fn parse_pubspec(root: &Path) -> Result<Vec<Package>> {
    let path = root.join("pubspec.lock");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path)?;

    let mut packages = Vec::new();
    let mut in_packages = false;
    let mut current_name = String::new();
    let mut current_version = String::new();

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed == "packages:" {
            in_packages = true;
            continue;
        }

        if in_packages {
            // Check if we're still in a package block (2-space indent = package name, 4-space = fields)
            if !line.starts_with("  ") || trimmed.is_empty() {
                // Flush
                if !current_name.is_empty() && !current_version.is_empty() {
                    packages.push(Package {
                        name: current_name.clone(),
                        version: current_version.clone(),
                        ecosystem: "Pub".to_string(),
                        source_file: path.clone(),
                    });
                }
                in_packages = !line.is_empty();
                current_name.clear();
                current_version.clear();
                continue;
            }

            // Package name line: "  package_name:"
            if line.starts_with("  ") && !line.starts_with("    ") && trimmed.ends_with(':') {
                // Flush previous
                if !current_name.is_empty() && !current_version.is_empty() {
                    packages.push(Package {
                        name: current_name.clone(),
                        version: current_version.clone(),
                        ecosystem: "Pub".to_string(),
                        source_file: path.clone(),
                    });
                }
                current_name = trimmed.trim_end_matches(':').to_string();
                current_version.clear();
            }

            // Version line: "    version: \"1.2.3\""
            if line.starts_with("    ") && trimmed.starts_with("version:") {
                current_version = trimmed
                    .strip_prefix("version:")
                    .unwrap_or("")
                    .trim()
                    .trim_matches('"')
                    .to_string();
            }
        }
    }

    // Flush last
    if !current_name.is_empty() && !current_version.is_empty() {
        packages.push(Package {
            name: current_name.clone(),
            version: current_version.clone(),
            ecosystem: "Pub".to_string(),
            source_file: path.clone(),
        });
    }

    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_npm() {
        let dir = std::env::temp_dir().join("lens-test-npm");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("package-lock.json"),
            r#"{
                "dependencies": {
                    "express": { "version": "4.18.2" },
                    "lodash": { "version": "4.17.20" }
                }
            }"#,
        )
        .unwrap();
        let pkgs = parse_npm(&dir).unwrap();
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].ecosystem, "npm");
        assert!(pkgs
            .iter()
            .any(|p| p.name == "express" && p.version == "4.18.2"));
    }

    #[test]
    fn test_parse_cargo() {
        let dir = std::env::temp_dir().join("lens-test-cargo");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("Cargo.lock"),
            r#"# This file is automatically @generated by Cargo.
[[package]]
name = "serde"
version = "1.0.195"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "1234"

[[package]]
name = "reqwest"
version = "0.11.24"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "5678"
"#,
        )
        .unwrap();
        let pkgs = parse_cargo(&dir).unwrap();
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].ecosystem, "crates.io");
        assert!(pkgs
            .iter()
            .any(|p| p.name == "serde" && p.version == "1.0.195"));
    }

    #[test]
    fn test_parse_gosum() {
        let dir = std::env::temp_dir().join("lens-test-go");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("go.sum"),
            "github.com/gin-gonic/gin v1.9.0 h1:abc123\ngithub.com/gin-gonic/gin v1.9.0/go.mod h1:def456\n",
        ).unwrap();
        let pkgs = parse_gosum(&dir).unwrap();
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].version, "1.9.0");
    }

    #[test]
    fn test_parse_pubspec() {
        let dir = std::env::temp_dir().join("lens-test-pub");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("pubspec.lock"),
            r#"packages:
  http:
    dependency: transitive
    description:
      name: http
      url: "https://pub.dev"
    version: "1.1.0"
  path:
    dependency: transitive
    description:
      name: path
      url: "https://pub.dev"
    version: "1.8.3"
sdks:
  dart: ">=3.0.0"
"#,
        )
        .unwrap();
        let pkgs = parse_pubspec(&dir).unwrap();
        assert_eq!(pkgs.len(), 2);
        assert!(pkgs
            .iter()
            .any(|p| p.name == "http" && p.version == "1.1.0"));
    }
}
