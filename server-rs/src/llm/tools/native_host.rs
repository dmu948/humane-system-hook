use std::convert::Infallible;
use std::env;
use std::io;
use std::process::Command;

use rig::completion::ToolDefinition;
use rig::tool::{Tool, ToolEmbedding};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const NATIVE_HOST_PATHS: [&str; 2] = [
    "/data/data/com.penumbraos.server/files/penumbra_tool_host",
    "/data/local/tmp/penumbraos/penumbra_tool_host",
];

#[derive(Clone, Default)]
pub struct WeatherGetTool;

#[derive(Debug, Deserialize)]
pub struct WeatherGetArgs {
    pub location: String,
}

#[derive(Debug, Serialize)]
pub struct NativeHostOutput {
    pub response: Value,
}

#[derive(Debug)]
pub struct NativeHostToolError(String);

impl std::fmt::Display for NativeHostToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NativeHostToolError {}

impl Tool for WeatherGetTool {
    const NAME: &'static str = "weather_get";

    type Error = NativeHostToolError;
    type Args = WeatherGetArgs;
    type Output = NativeHostOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Get current weather for a named location using the local PenumbraOS native tool host. Use this for weather questions such as current conditions, temperature, humidity, wind, rain, or whether the user needs a jacket. The location can be a city, city and state, ZIP code, or place name.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "Weather location, such as Fairfax, VA; New York; 22030; or the user's stated place."
                    }
                },
                "required": ["location"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        call_native_host(&["weather", args.location.as_str()])
    }
}

impl ToolEmbedding for WeatherGetTool {
    type InitError = Infallible;
    type Context = ();
    type State = ();

    fn embedding_docs(&self) -> Vec<String> {
        vec![
            "Get live weather by location, city, state, ZIP code, or place name. Use for prompts like: check the weather in Fairfax, what is the temperature outside, is it raining, do I need a jacket, what is the humidity, what is the wind like.".to_string(),
        ]
    }

    fn context(&self) -> Self::Context {}

    fn init(_state: Self::State, _context: Self::Context) -> Result<Self, Self::InitError> {
        Ok(Self)
    }
}

fn call_native_host(args: &[&str]) -> Result<NativeHostOutput, NativeHostToolError> {
    let output = run_native_host(args)?.1;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        return Err(NativeHostToolError(format!(
            "native tool host exited with status {}: {}",
            output
                .status
                .code()
                .map_or_else(|| "signal".to_string(), |code| code.to_string()),
            if stderr.is_empty() { stdout } else { stderr }
        )));
    }

    let response = serde_json::from_str(&stdout).map_err(|err| {
        NativeHostToolError(format!(
            "native tool host returned invalid JSON: {err}; output={stdout}"
        ))
    })?;

    Ok(NativeHostOutput { response })
}

fn run_native_host(args: &[&str]) -> Result<(String, std::process::Output), NativeHostToolError> {
    let mut errors = Vec::new();

    for path in native_host_paths() {
        match Command::new(&path).arg("--json").args(args).output() {
            Ok(output) => return Ok((path, output)),
            Err(err) if is_missing_or_unlaunchable(&err) => {
                errors.push(format!("{path}: {err}"));
            }
            Err(err) => {
                return Err(NativeHostToolError(format!(
                    "failed to run native tool host at {path}: {err}"
                )));
            }
        }
    }

    Err(NativeHostToolError(format!(
        "failed to run native tool host; tried {}",
        errors.join("; ")
    )))
}

fn native_host_paths() -> Vec<String> {
    let mut paths = Vec::new();
    if let Ok(path) = env::var("PENUMBRA_TOOL_HOST_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            paths.push(trimmed.to_string());
        }
    }
    paths.extend(NATIVE_HOST_PATHS.iter().map(|path| path.to_string()));
    paths
}

fn is_missing_or_unlaunchable(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_host_paths_are_ordered() {
        assert_eq!(
            native_host_paths(),
            [
                "/data/data/com.penumbraos.server/files/penumbra_tool_host".to_string(),
                "/data/local/tmp/penumbraos/penumbra_tool_host".to_string()
            ]
        );
    }

    #[test]
    fn not_found_errors_fall_through_to_next_path() {
        let err = io::Error::from(io::ErrorKind::NotFound);

        assert!(is_missing_or_unlaunchable(&err));
    }

    #[test]
    fn permission_denied_errors_fall_through_to_next_path() {
        let err = io::Error::from(io::ErrorKind::PermissionDenied);

        assert!(is_missing_or_unlaunchable(&err));
    }

    #[test]
    fn other_launch_errors_stop_path_fallback() {
        let err = io::Error::from(io::ErrorKind::InvalidInput);

        assert!(!is_missing_or_unlaunchable(&err));
    }

    #[test]
    fn fallback_error_mentions_both_paths() {
        let err = run_native_host(&["health"]).unwrap_err().to_string();

        assert!(err.contains("/data/data/com.penumbraos.server/files/penumbra_tool_host"));
        assert!(err.contains("/data/local/tmp/penumbraos/penumbra_tool_host"));
    }

    #[tokio::test]
    async fn weather_definition_uses_expected_name() {
        let definition = WeatherGetTool.definition(String::new()).await;

        assert_eq!(definition.name, "weather_get");
        assert!(definition.description.contains("weather"));
        assert!(definition.parameters.to_string().contains("location"));
    }
}
