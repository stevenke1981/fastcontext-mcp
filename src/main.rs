use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Component, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::runtime::Builder;
use tokio::time::timeout;

const SERVER_NAME: &str = "fastcontext-mcp-rust";
const DEFAULT_PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: Option<String>,
    params: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Deserialize)]
struct ExploreArgs {
    /// Natural-language repository exploration request.
    query: String,
    /// Repository directory. Must be inside FASTCONTEXT_ALLOWED_ROOT.
    work_dir: Option<String>,
    /// Maximum FastContext exploration turns.
    max_turns: Option<u32>,
    /// Return only <final_answer> citation block when possible. Default: true.
    citation: Option<bool>,
    /// Relative JSONL trajectory file path under work_dir.
    trajectory_file: Option<String>,
    /// Override command timeout in seconds.
    timeout_secs: Option<u64>,
    /// Ask FastContext CLI to print intermediate messages.
    verbose: Option<bool>,
    /// Override BASE_URL for this request (e.g. http://127.0.0.1:30000/v1).
    base_url: Option<String>,
    /// Override MODEL for this request (e.g. microsoft/FastContext-1.0-4B-RL).
    model: Option<String>,
    /// Override API_KEY for this request.
    api_key: Option<String>,
}

#[derive(Clone, Debug)]
struct Config {
    fastcontext_bin: String,
    default_work_dir: PathBuf,
    allowed_root: PathBuf,
    default_max_turns: u32,
    default_timeout_secs: u64,
}

impl Config {
    fn from_env() -> Result<Self> {
        let fastcontext_bin =
            env::var("FASTCONTEXT_BIN").unwrap_or_else(|_| "fastcontext".to_string());
        let default_work_dir = env::var("FASTCONTEXT_WORK_DIR")
            .map(PathBuf::from)
            .unwrap_or(env::current_dir()?);
        let allowed_root = env::var("FASTCONTEXT_ALLOWED_ROOT")
            .map(PathBuf::from)
            .unwrap_or(default_work_dir.clone());
        let default_max_turns = env::var("FASTCONTEXT_MAX_TURNS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(6);
        let default_timeout_secs = env::var("FASTCONTEXT_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(300);

        Ok(Self {
            fastcontext_bin,
            default_work_dir,
            allowed_root,
            default_max_turns,
            default_timeout_secs,
        })
    }

    fn resolve_work_dir(&self, input: Option<&str>) -> Result<PathBuf> {
        let raw = input
            .map(PathBuf::from)
            .unwrap_or_else(|| self.default_work_dir.clone());

        let canonical = raw
            .canonicalize()
            .with_context(|| format!("cannot canonicalize work_dir: {}", raw.display()))?;
        let allowed = self.allowed_root.canonicalize().with_context(|| {
            format!(
                "cannot canonicalize FASTCONTEXT_ALLOWED_ROOT: {}",
                self.allowed_root.display()
            )
        })?;

        if !canonical.starts_with(&allowed) {
            bail!(
                "work_dir is outside allowed root. work_dir={}, allowed_root={}",
                canonical.display(),
                allowed.display()
            );
        }

        Ok(canonical)
    }
}

fn response(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error_response(id: Option<Value>, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(Value::Null),
        "error": {"code": code, "message": message.into()}
    })
}

fn write_json(value: &Value) -> Result<()> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer(&mut stdout, value)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn tools_list_result() -> Value {
    json!({
        "tools": [
            {
                "name": "fastcontext_explore",
                "description": "Explore a repository using Microsoft FastContext CLI with the configured FastContext-1.0-4B-RL endpoint. Read-only; returns compact file paths and line ranges.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Natural-language request, e.g. 'Find where authentication middleware is implemented'."
                        },
                        "work_dir": {
                            "type": "string",
                            "description": "Repository directory. Must be inside FASTCONTEXT_ALLOWED_ROOT. Defaults to FASTCONTEXT_WORK_DIR."
                        },
                        "max_turns": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 20,
                            "default": 6
                        },
                        "citation": {
                            "type": "boolean",
                            "default": true,
                            "description": "Ask FastContext to return only the machine-readable <final_answer> citation block."
                        },
                        "trajectory_file": {
                            "type": "string",
                            "default": ".fastcontext/trajectory.jsonl",
                            "description": "Relative path under work_dir for FastContext trajectory JSONL."
                        },
                        "timeout_secs": {
                            "type": "integer",
                            "minimum": 10,
                            "maximum": 1800,
                            "default": 300
                        },
                        "verbose": {
                            "type": "boolean",
                            "default": false
                        },
                        "base_url": {
                            "type": "string",
                            "description": "Override BASE_URL for this request, e.g. http://127.0.0.1:30000/v1"
                        },
                        "model": {
                            "type": "string",
                            "description": "Override MODEL for this request, e.g. microsoft/FastContext-1.0-4B-RL"
                        },
                        "api_key": {
                            "type": "string",
                            "description": "Override API_KEY for this request"
                        }
                    },
                    "required": ["query"],
                    "additionalProperties": false
                }
            },
            {
                "name": "fastcontext_status",
                "description": "Check the MCP server configuration and fastcontext CLI availability. Read-only diagnostic tool.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": [],
                    "additionalProperties": false
                }
            }
        ]
    })
}

fn validate_relative_path(path: &str) -> Result<PathBuf> {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        bail!("trajectory_file must be a relative path");
    }
    for c in p.components() {
        match c {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("trajectory_file must stay under work_dir")
            }
            _ => {}
        }
    }
    Ok(p)
}

/// Check if the fastcontext binary is available on PATH.
fn check_fastcontext_binary(name: &str) -> (bool, String) {
    match std::process::Command::new(name).arg("--help").output() {
        Ok(_) => (true, "found at PATH or cwd".to_string()),
        Err(e) => (false, format!("{e}")),
    }
}

/// Build a server status report string (used by fastcontext_status tool
/// and startup diagnostics).
fn server_status_text(config: &Config) -> String {
    let (bin_ok, bin_detail) = check_fastcontext_binary(&config.fastcontext_bin);

    let base_url = env::var("BASE_URL").ok();
    let model = env::var("MODEL").ok();
    let api_key_set = env::var("API_KEY").ok().map(|v| !v.is_empty());

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "server:     {} v{}",
        SERVER_NAME,
        env!("CARGO_PKG_VERSION")
    ));
    lines.push(format!(
        "binary:     {} {}",
        config.fastcontext_bin,
        if bin_ok { "✓" } else { "✗ NOT FOUND" }
    ));
    lines.push(format!("binary_detail: {}", bin_detail));
    lines.push(format!("work_dir:   {}", config.default_work_dir.display()));
    lines.push(format!("allowed_root: {}", config.allowed_root.display()));
    lines.push(format!("max_turns:  {}", config.default_max_turns));
    lines.push(format!("timeout:    {}s", config.default_timeout_secs));
    match base_url {
        Some(ref u) => lines.push(format!("BASE_URL:   {}", u)),
        None => lines.push("BASE_URL:   (not set) ⚠".to_string()),
    }
    match model {
        Some(ref m) => lines.push(format!("MODEL:      {}", m)),
        None => lines.push("MODEL:      (not set) ⚠".to_string()),
    }
    match api_key_set {
        Some(true) => lines.push("API_KEY:    ✓ (set)".to_string()),
        Some(false) => lines.push("API_KEY:    (empty) ⚠".to_string()),
        None => lines.push("API_KEY:    (not set) — OK for local servers".to_string()),
    }
    lines.join("\n")
}

fn truncate_for_error(text: &[u8], max: usize) -> String {
    let s = String::from_utf8_lossy(text).replace('\n', "\\n");
    if s.len() > max {
        format!("{}...", &s[..max])
    } else {
        s
    }
}

fn run_fastcontext(config: &Config, args: ExploreArgs) -> Result<String> {
    let rt = Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to create tokio runtime")?;
    rt.block_on(run_fastcontext_async(config, args))
}

async fn run_fastcontext_async(config: &Config, args: ExploreArgs) -> Result<String> {
    let query = args.query.trim();
    if query.is_empty() {
        bail!("query cannot be empty");
    }
    if query.len() > 8000 {
        bail!("query is too long; keep it focused");
    }

    let work_dir = config.resolve_work_dir(args.work_dir.as_deref())?;
    let max_turns = args
        .max_turns
        .unwrap_or(config.default_max_turns)
        .clamp(1, 20);
    let citation = args.citation.unwrap_or(true);
    let timeout_secs = args
        .timeout_secs
        .unwrap_or(config.default_timeout_secs)
        .clamp(10, 1800);
    let trajectory_rel = validate_relative_path(
        args.trajectory_file
            .as_deref()
            .unwrap_or(".fastcontext/trajectory.jsonl"),
    )?;

    if let Some(parent) = trajectory_rel.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(work_dir.join(parent))?;
        }
    }

    let mut cmd = Command::new(&config.fastcontext_bin);
    cmd.kill_on_drop(true)
        .current_dir(&work_dir)
        .arg("--query")
        .arg(query)
        .arg("--max-turns")
        .arg(max_turns.to_string())
        .arg("--traj")
        .arg(&trajectory_rel)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if citation {
        cmd.arg("--citation");
    }
    if args.verbose.unwrap_or(false) {
        cmd.arg("--verbose");
    }

    // Pass per-request endpoint overrides as environment variables.
    // fastcontext CLI reads BASE_URL, MODEL, API_KEY from its own environment.
    if let Some(ref url) = args.base_url {
        if !url.is_empty() {
            cmd.env("BASE_URL", url);
        }
    }
    if let Some(ref model) = args.model {
        if !model.is_empty() {
            cmd.env("MODEL", model);
        }
    }
    if let Some(ref key) = args.api_key {
        if !key.is_empty() {
            cmd.env("API_KEY", key);
        }
    }

    let output = timeout(Duration::from_secs(timeout_secs), cmd.output())
        .await
        .map_err(|_| anyhow!("fastcontext timed out after {timeout_secs}s"))?
        .with_context(|| {
            format!(
                "failed to spawn or wait for FastContext CLI: {}",
                config.fastcontext_bin
            )
        })?;

    if !output.status.success() {
        bail!(
            "fastcontext failed with status {}. stderr={}",
            output.status,
            truncate_for_error(&output.stderr, 2000)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        bail!("fastcontext returned empty output");
    }

    Ok(stdout)
}

fn handle_tools_call(config: &Config, params: Option<Value>) -> Value {
    let parsed: ToolCallParams = match serde_json::from_value(params.unwrap_or(Value::Null)) {
        Ok(v) => v,
        Err(err) => {
            return json!({
                "content": [{"type": "text", "text": format!("invalid tools/call params: {err}")}],
                "isError": true
            });
        }
    };

    match parsed.name.as_str() {
        "fastcontext_explore" => {
            let args: ExploreArgs = match serde_json::from_value(parsed.arguments) {
                Ok(v) => v,
                Err(err) => {
                    return json!({
                        "content": [{"type": "text", "text": format!("invalid fastcontext_explore arguments: {err}")}],
                        "isError": true
                    });
                }
            };

            match run_fastcontext(config, args) {
                Ok(text) => json!({"content": [{"type": "text", "text": text}], "isError": false}),
                Err(err) => {
                    let msg = err.to_string();
                    // Improve error message for common "binary not found" case
                    let enriched = if msg.contains("program not found")
                        || msg.contains("No such file")
                        || msg.to_lowercase().contains("cannot find")
                    {
                        format!(
                            "{}\n\nHint: Install fastcontext CLI: uv tool install . from https://github.com/microsoft/fastcontext\nOr set FASTCONTEXT_BIN env var to the correct path.",
                            msg
                        )
                    } else {
                        msg
                    };
                    json!({"content": [{"type": "text", "text": enriched}], "isError": true})
                }
            }
        }
        "fastcontext_status" => {
            let report = server_status_text(config);
            json!({"content": [{"type": "text", "text": report}], "isError": false})
        }
        other => json!({
            "content": [{"type": "text", "text": format!("unknown tool: {other}")}],
            "isError": true
        }),
    }
}

fn handle_request(config: &Config, req: JsonRpcRequest) -> Option<Value> {
    let method = req.method.unwrap_or_default();

    // Notifications do not require responses.
    req.id.as_ref()?;
    let id = req.id.clone().unwrap_or(Value::Null);

    let result = match method.as_str() {
        "initialize" => {
            let requested_version = req
                .params
                .as_ref()
                .and_then(|p| p.get("protocolVersion"))
                .and_then(|v| v.as_str())
                .unwrap_or(DEFAULT_PROTOCOL_VERSION);

            json!({
                "protocolVersion": requested_version,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION")}
            })
        }
        "tools/list" => tools_list_result(),
        "tools/call" => handle_tools_call(config, req.params),
        "ping" => json!({}),
        _ => {
            return Some(error_response(
                Some(id),
                -32601,
                format!("method not found: {method}"),
            ))
        }
    };

    Some(response(id, result))
}

fn main() -> Result<()> {
    let config = Config::from_env()?;
    let stdin = io::stdin();

    // Startup diagnostics
    let (bin_ok, bin_detail) = check_fastcontext_binary(&config.fastcontext_bin);
    let bin_status = if bin_ok { "✓" } else { "✗ NOT FOUND" };
    let base_url_status = match env::var("BASE_URL") {
        Ok(ref u) if !u.is_empty() => format!("✓ ({u})"),
        _ => "⚠ NOT SET — fastcontext will fail".to_string(),
    };
    let model_status = match env::var("MODEL") {
        Ok(ref m) if !m.is_empty() => format!("✓ ({m})"),
        _ => "⚠ NOT SET — fastcontext will fail".to_string(),
    };

    eprintln!("{SERVER_NAME} v{}", env!("CARGO_PKG_VERSION"));
    eprintln!("  binary:     {} {}", config.fastcontext_bin, bin_status);
    if !bin_ok {
        eprintln!("  binary_detail: {}", bin_detail);
        eprintln!("  HINT: Install fastcontext CLI or set FASTCONTEXT_BIN");
    }
    eprintln!("  BASE_URL:   {}", base_url_status);
    eprintln!("  MODEL:      {}", model_status);
    eprintln!("  work_dir:   {}", config.default_work_dir.display());
    eprintln!("  allowed:    {}", config.allowed_root.display());
    eprintln!("  max_turns:  {}", config.default_max_turns);
    eprintln!("  timeout:    {}s", config.default_timeout_secs);
    if !bin_ok {
        eprintln!("WARNING: fastcontext binary not found. Tool calls will fail until installed.");
    }

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(err) => {
                write_json(&error_response(None, -32700, format!("parse error: {err}")))?;
                continue;
            }
        };

        if let Some(out) = handle_request(&config, req) {
            write_json(&out)?;
        }
    }

    Err(anyhow!("stdin closed"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_config() -> Config {
        Config {
            fastcontext_bin: "fastcontext".to_string(),
            default_work_dir: PathBuf::from("."),
            allowed_root: PathBuf::from("."),
            default_max_turns: 6,
            default_timeout_secs: 300,
        }
    }

    // -- validate_relative_path --

    #[test]
    fn test_validate_relative_path_valid_simple() {
        let result = validate_relative_path("file.jsonl");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_relative_path_valid_nested() {
        let result = validate_relative_path("a/b/c.jsonl");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("a/b/c.jsonl"));
    }

    #[test]
    fn test_validate_relative_path_rejects_absolute() {
        let result = validate_relative_path("/etc/passwd");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("work_dir") || msg.contains("relative") || msg.contains("outside"));
    }

    #[test]
    fn test_validate_relative_path_rejects_parent_dir() {
        let result = validate_relative_path("../escape.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_relative_path_rejects_root() {
        let result = validate_relative_path("/");
        assert!(result.is_err());
    }

    // -- truncate_for_error --

    #[test]
    fn test_truncate_short_text_unchanged() {
        let result = truncate_for_error(b"hello", 100);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_long_text() {
        let result = truncate_for_error(b"hello world", 5);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn test_truncate_newlines_converted() {
        let result = truncate_for_error(b"line1\nline2", 100);
        assert_eq!(result, "line1\\nline2");
    }

    #[test]
    fn test_truncate_empty() {
        let result = truncate_for_error(b"", 10);
        assert_eq!(result, "");
    }

    #[test]
    fn test_truncate_exact_boundary() {
        let result = truncate_for_error(b"abcde", 5);
        assert_eq!(result, "abcde");
    }

    // -- tools_list_result --

    #[test]
    fn test_tools_list_has_tools_key() {
        let result = tools_list_result();
        assert!(result.get("tools").is_some());
    }

    #[test]
    fn test_tools_list_contains_fastcontext_explore() {
        let result = tools_list_result();
        let tools = result["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"fastcontext_explore"));
    }

    #[test]
    fn test_tools_list_input_schema_has_query_required() {
        let result = tools_list_result();
        let tool = &result["tools"][0];
        let schema = &tool["inputSchema"];
        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&json!("query")));
    }

    #[test]
    fn test_tools_list_contains_fastcontext_status() {
        let result = tools_list_result();
        let tools = result["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"fastcontext_status"));
    }

    // -- response / error_response --

    #[test]
    fn test_response_structure() {
        let result = response(json!(1), json!({"key": "val"}));
        assert_eq!(result["jsonrpc"], "2.0");
        assert_eq!(result["id"], 1);
        assert_eq!(result["result"]["key"], "val");
    }

    #[test]
    fn test_error_response_with_id() {
        let result = error_response(Some(json!(42)), -32601, "not found");
        assert_eq!(result["jsonrpc"], "2.0");
        assert_eq!(result["id"], 42);
        assert_eq!(result["error"]["code"], -32601);
        assert_eq!(result["error"]["message"], "not found");
    }

    #[test]
    fn test_error_response_null_id() {
        let result = error_response(None, -32700, "parse error");
        assert_eq!(result["id"], Value::Null);
    }

    // -- ExploreArgs deserialization --

    #[test]
    fn test_explore_args_minimal() {
        let json = json!({"name": "fastcontext_explore", "arguments": {"query": "find auth"}});
        let call: ToolCallParams = serde_json::from_value(json).unwrap();
        assert_eq!(call.name, "fastcontext_explore");
        assert_eq!(call.arguments["query"], "find auth");
    }

    #[test]
    fn test_explore_args_all_fields() {
        let json = json!({
            "query": "find routes",
            "work_dir": "/repo",
            "max_turns": 10,
            "citation": false,
            "trajectory_file": "traj.jsonl",
            "timeout_secs": 120,
            "verbose": true,
            "base_url": "http://localhost:8080/v1",
            "model": "my-model",
            "api_key": "sk-test"
        });
        let args: ExploreArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.query, "find routes");
        assert_eq!(args.work_dir.unwrap(), "/repo");
        assert_eq!(args.max_turns.unwrap(), 10);
        assert!(!args.citation.unwrap());
        assert_eq!(args.base_url.unwrap(), "http://localhost:8080/v1");
        assert_eq!(args.model.unwrap(), "my-model");
        assert_eq!(args.api_key.unwrap(), "sk-test");
    }

    #[test]
    fn test_explore_args_defaults() {
        let json = json!({"query": "search"});
        let args: ExploreArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.query, "search");
        assert!(args.work_dir.is_none());
        assert!(args.max_turns.is_none());
        assert!(args.citation.is_none());
    }

    // -- JSON-RPC request handling --

    #[test]
    fn test_handle_initialize() {
        let config = test_config();
        let req = JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(json!(1)),
            method: Some("initialize".to_string()),
            params: Some(json!({"protocolVersion": "2024-11-05"})),
        };
        let result = handle_request(&config, req);
        assert!(result.is_some());
        let val = result.unwrap();
        assert_eq!(val["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(val["result"]["serverInfo"]["name"], "fastcontext-mcp-rust");
    }

    #[test]
    fn test_handle_tools_list() {
        let config = test_config();
        let req = JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(json!(1)),
            method: Some("tools/list".to_string()),
            params: None,
        };
        let result = handle_request(&config, req);
        assert!(result.is_some());
        let val = result.unwrap();
        assert!(val["result"]["tools"].is_array());
    }

    #[test]
    fn test_handle_ping() {
        let config = test_config();
        let req = JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(json!(1)),
            method: Some("ping".to_string()),
            params: None,
        };
        let result = handle_request(&config, req);
        assert!(result.is_some());
    }

    #[test]
    fn test_handle_unknown_method_returns_error() {
        let config = test_config();
        let req = JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(json!(1)),
            method: Some("bogus".to_string()),
            params: None,
        };
        let result = handle_request(&config, req);
        assert!(result.is_some());
        let val = result.unwrap();
        assert!(val.get("error").is_some());
    }

    #[test]
    fn test_handle_notification_returns_none() {
        let config = test_config();
        let req = JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: None, // notification: no id
            method: Some("ping".to_string()),
            params: None,
        };
        let result = handle_request(&config, req);
        assert!(result.is_none());
    }

    #[test]
    fn test_handle_tools_call_invalid_params() {
        let config = test_config();
        let req = JsonRpcRequest {
            jsonrpc: Some("2.0".to_string()),
            id: Some(json!(1)),
            method: Some("tools/call".to_string()),
            params: Some(json!({"garbage": "data"})),
        };
        let result = handle_request(&config, req);
        assert!(result.is_some());
    }

    // -- Config::resolve_work_dir --

    // -- server_status_text --

    #[test]
    fn test_server_status_text_includes_server_name() {
        let cfg = test_config();
        let report = server_status_text(&cfg);
        assert!(report.contains("fastcontext-mcp-rust"));
        assert!(report.contains("BASE_URL"));
        assert!(report.contains("MODEL"));
        assert!(report.contains("binary"));
    }

    #[test]
    fn test_server_status_text_contains_config_values() {
        let cfg = test_config();
        let report = server_status_text(&cfg);
        assert!(report.contains("fastcontext")); // binary name
        assert!(report.contains("max_turns"));
        assert!(report.contains("timeout"));
    }

    // -- check_fastcontext_binary --

    #[test]
    fn test_check_binary_nonexistent_returns_false() {
        let (found, _detail) = check_fastcontext_binary("this-binary-does-not-exist-hopefully");
        assert!(!found);
    }

    #[test]
    fn test_resolve_work_dir_default() {
        let config = Config {
            fastcontext_bin: "fastcontext".to_string(),
            default_work_dir: PathBuf::from("."),
            allowed_root: PathBuf::from("."),
            default_max_turns: 6,
            default_timeout_secs: 300,
        };
        let result = config.resolve_work_dir(None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_work_dir_outside_allowed() {
        // Use temp dir outside cwd as disallowed path
        let config = Config {
            fastcontext_bin: "fastcontext".to_string(),
            default_work_dir: PathBuf::from("."),
            allowed_root: PathBuf::from("."),
            default_max_turns: 6,
            default_timeout_secs: 300,
        };
        // An absolute path that is unlikely to be under "." canonicalized
        let result = config.resolve_work_dir(Some("C:\\Windows\\System32"));
        assert!(result.is_err());
    }
}
