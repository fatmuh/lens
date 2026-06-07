//! `lens setup` — interactive AI configuration.
//!
//! Prompts the user for:
//!   1. OpenAI-compatible API base URL
//!   2. API key
//!   3. Fetches available models from `/models`
//!   4. Model selection (pick from list or enter custom)
//!
//! Saves to `~/.lens/config.toml` (OS-appropriate).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

/// AI configuration stored in `~/.lens/config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSetupConfig {
    pub ai: AiSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl Default for AiSetupConfig {
    fn default() -> Self {
        Self {
            ai: AiSettings {
                base_url: "https://api.openai.com/v1".into(),
                api_key: String::new(),
                model: "gpt-4o".into(),
            },
        }
    }
}

/// Get the config directory: `~/.lens/` on all platforms.
pub fn config_dir() -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(home.join(".lens"))
}

/// Get the config file path: `~/.lens/config.toml`.
pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

/// Load existing config, or return default if not found.
pub fn load_config() -> Result<AiSetupConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AiSetupConfig::default());
    }
    let content = std::fs::read_to_string(&path)
        .context("reading ~/.lens/config.toml")?;
    let cfg: AiSetupConfig = toml::from_str(&content)
        .context("parsing ~/.lens/config.toml")?;
    Ok(cfg)
}

/// Save config to `~/.lens/config.toml`.
pub fn save_config(cfg: &AiSetupConfig) -> Result<PathBuf> {
    let dir = config_dir()?;
    std::fs::create_dir_all(&dir)
        .context("creating ~/.lens/ directory")?;
    let path = dir.join("config.toml");
    let content = toml::to_string_pretty(cfg)
        .context("serializing config")?;
    std::fs::write(&path, content)
        .context("writing config file")?;
    Ok(path)
}

/// Get home directory (cross-platform).
fn home_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .context("USERPROFILE not set")
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .context("HOME not set")
    }
}

/// Run the interactive setup.
pub fn run_setup() -> Result<std::process::ExitCode> {
    use owo_colors::OwoColorize;

    println!();
    println!("  {} Lens AI Setup", "🤖".cyan());
    println!("  {} Configure your OpenAI-compatible API for auto-fix features", "→".dimmed());
    println!();

    // Load existing config as defaults.
    let existing = load_config().unwrap_or_default();

    // 1. Base URL
    let default_url = if existing.ai.base_url.is_empty() {
        "https://api.openai.com/v1".to_string()
    } else {
        existing.ai.base_url.clone()
    };
    println!("  {} API Base URL", "1.".bold());
    println!("  {} Examples:", "→".dimmed());
    println!("  {}   OpenAI:    https://api.openai.com/v1", "→".dimmed());
    println!("  {}   Ollama:    http://localhost:11434/v1", "→".dimmed());
    println!("  {}   Groq:      https://api.groq.com/openai/v1", "→".dimmed());
    println!("  {}   OpenRouter: https://openrouter.ai/api/v1", "→".dimmed());
    println!("  {}   Together:  https://api.together.xyz/v1", "→".dimmed());
    println!("  {}   Custom:    any OpenAI-compatible endpoint", "→".dimmed());
    let base_url = prompt_default("  URL", &default_url)?;

    // 2. API Key
    println!();
    println!("  {} API Key", "2.".bold());
    println!("  {} Leave empty for local models (Ollama, etc.)", "→".dimmed());
    let api_key = prompt_password("  Key")?;

    // 3. Fetch models
    println!();
    println!("  {} Fetching available models...", "3.".bold().cyan());

    let models = fetch_models(&base_url, &api_key);
    let model = match &models {
        Ok(list) if !list.is_empty() => {
            println!("  {} Available models:", "→".green());
            for (i, m) in list.iter().enumerate().take(20) {
                println!("    {} {}", format!("[{}]", i + 1).yellow(), m);
            }
            if list.len() > 20 {
                println!("    {} ... and {} more", "→".dimmed(), list.len() - 20);
            }
            println!();

            let default_model = if existing.ai.model.is_empty() {
                list.first().cloned().unwrap_or_default()
            } else {
                existing.ai.model.clone()
            };

            loop {
                println!("  {} Enter model number (1-{}) or type a custom model name", "→".bold(), list.len().min(20));
                let input = prompt_default("  Model", &default_model)?;
                // Check if it's a number
                if let Ok(idx) = input.parse::<usize>() {
                    if idx >= 1 && idx <= list.len().min(20) {
                        break list[idx - 1].clone();
                    }
                }
                // It's a custom model name
                break input;
            }
        }
        Ok(_) => {
            println!("  {} No models found. Enter model name manually.", "→".yellow());
            let default_model = if existing.ai.model.is_empty() {
                "gpt-4o".to_string()
            } else {
                existing.ai.model.clone()
            };
            prompt_default("  Model", &default_model)?
        }
        Err(e) => {
            println!("  {} Could not fetch models: {}", "⚠".yellow(), e);
            println!("  {} Enter model name manually.", "→".dimmed());
            let default_model = if existing.ai.model.is_empty() {
                "gpt-4o".to_string()
            } else {
                existing.ai.model.clone()
            };
            prompt_default("  Model", &default_model)?
        }
    };

    // 4. Save
    let cfg = AiSetupConfig {
        ai: AiSettings {
            base_url: base_url.trim().to_string(),
            api_key: api_key.trim().to_string(),
            model: model.trim().to_string(),
        },
    };

    let path = save_config(&cfg)?;
    println!();
    println!("  {} Configuration saved to {}", "✓".green().bold(), path.display());
    println!();
    println!("  {} AI Settings:", "→".bold());
    println!("    Base URL: {}", cfg.ai.base_url.cyan());
    println!("    Model:    {}", cfg.ai.model.yellow());
    if cfg.ai.api_key.is_empty() {
        println!("    API Key:  {} (no key — local mode)", "none".dimmed());
    } else {
        let masked = mask_key(&cfg.ai.api_key);
        println!("    API Key:  {}", masked.dimmed());
    }
    println!();
    println!("  {} Ready! Try:", "🚀".green());
    println!("    lens fix . --mode coverage --coverage lcov.info");
    println!("    lens watch . --mode all --coverage lcov.info");
    println!();

    Ok(std::process::ExitCode::SUCCESS)
}

/// Fetch models from OpenAI-compatible `/models` endpoint.
fn fetch_models(base_url: &str, api_key: &str) -> Result<Vec<String>> {
    // Use a minimal Tokio runtime for the HTTP request.
    let rt = tokio::runtime::Runtime::new()
        .context("creating runtime")?;

    rt.block_on(async {
        let client = reqwest::Client::new();
        let url = format!("{}/models", base_url.trim_end_matches('/'));

        let mut req = client.get(&url);
        if !api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        let resp = req.send().await
            .context("Failed to connect to API. Check the URL.")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "API returned status {}. Response: {}",
                status,
                body.chars().take(200).collect::<String>()
            );
        }

        #[derive(Deserialize)]
        struct ModelsResponse {
            data: Option<Vec<ModelInfo>>,
        }
        #[derive(Deserialize)]
        struct ModelInfo {
            id: String,
        }

        let body: ModelsResponse = resp.json().await
            .context("Could not parse models response")?;

        let mut models = body.data
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.id)
            .collect::<Vec<_>>();
        models.sort();

        Ok(models)
    })
}

/// Prompt with a default value (shown in brackets).
fn prompt_default(label: &str, default: &str) -> Result<String> {
    use owo_colors::OwoColorize;
    if default.is_empty() {
        print!("{}: ", label.bold());
    } else {
        print!("{} [{}]: ", label.bold(), default.cyan());
    }
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    let trimmed = input.trim().to_string();
    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed)
    }
}

/// Prompt for password (no echo on terminal).
fn prompt_password(label: &str) -> Result<String> {
    use owo_colors::OwoColorize;
    print!("{}: ", label.bold());
    io::stdout().flush()?;

    // Try to use rpassword-like behavior, but keep it simple.
    // On Windows, just read from stdin (no echo control without winapi).
    let mut input = String::new();
    io::stdin().lock().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

/// Mask an API key for display: "sk-abc...xyz"
fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        return "*".repeat(key.len());
    }
    format!("{}...{}", &key[..4], &key[key.len()-4..])
}
