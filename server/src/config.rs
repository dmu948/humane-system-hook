use serde::Deserialize;
use std::path::Path;
use tracing::info;

/// Top-level configuration, loaded from `config.toml`.
#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub server: ServerConfig,
}

#[derive(Debug, Deserialize)]
pub struct LlmConfig {
    /// Provider name: "gemini", "anthropic", "openai", "openai-compatible", "echo"
    #[serde(default = "default_provider")]
    pub provider: String,

    /// Model ID for the chosen provider (e.g. "gemini-2.5-flash")
    #[serde(default = "default_model")]
    pub model: String,

    /// API key — overrides the corresponding env var if set.
    pub api_key: Option<String>,

    /// Base URL — only used for "openai-compatible" provider.
    pub base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    /// gRPC listen port.
    #[serde(default = "default_port")]
    pub port: u16,

    /// System prompt sent to the LLM.
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
}

// --- defaults ---

fn default_provider() -> String {
    "echo".into()
}

fn default_model() -> String {
    "gemini-2.5-flash".into()
}

fn default_port() -> u16 {
    9090
}

fn default_system_prompt() -> String {
    "You are a helpful assistant running on a Humane AI Pin. Keep responses concise - they will be displayed on a laser projector and spoken aloud.".into()
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            model: default_model(),
            api_key: None,
            base_url: None,
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            system_prompt: default_system_prompt(),
        }
    }
}

impl Config {
    /// Load config from file. Falls back to defaults if file is missing.
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            let config: Config = toml::from_str(&contents)?;
            info!(?path, "loaded config");
            Ok(config)
        } else {
            info!(?path, "config file not found, using defaults");
            Ok(Config {
                llm: LlmConfig::default(),
                server: ServerConfig::default(),
            })
        }
    }
}

impl LlmConfig {
    /// Resolve the API key: config file value takes priority, then env var.
    pub fn resolve_api_key(&self) -> Option<String> {
        if let Some(ref key) = self.api_key {
            if !key.is_empty() {
                return Some(key.clone());
            }
        }

        None
    }
}
