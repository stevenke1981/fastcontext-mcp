use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Component, Path, PathBuf};

const SERVER_NAME: &str = "fastcontext-mcp-rust";
const DEFAULT_PROTOCOL_VERSION: &str = "2024-11-05";

// ============================================================
// JSON-RPC types
// ============================================================

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

// ============================================================
// Domain types
// ============================================================

#[derive(Debug, Deserialize)]
struct ExploreArgs {
    /// Natural-language repository exploration request.
    query: String,
    /// Repository directory. Must be inside FASTCONTEXT_ALLOWED_ROOT.
    work_dir: Option<String>,
    /// Maximum exploration turns (LLM tool-call iterations).
    max_turns: Option<u32>,
    /// Override command timeout in seconds.
    timeout_secs: Option<u64>,
    /// Override BASE_URL for this request (e.g. http://127.0.0.1:30000/v1).
    base_url: Option<String>,
    /// Override MODEL for this request (e.g. microsoft/FastContext-1.0-4B-RL).
    model: Option<String>,
    /// Override API_KEY for this request.
    api_key: Option<String>,
}

#[derive(Clone, Debug)]
struct Config {
    base_url: String,
    model: String,
    api_key: String,
    default_work_dir: PathBuf,
    allowed_root: PathBuf,
    default_max_turns: u32,
    default_timeout_secs: u64,
}

impl Config {
    fn from_env() -> Result<Self> {
        let base_url = env::var("BASE_URL").context("BASE_URL environment variable is required")?;
        let model = env::var("MODEL").context("MODEL environment variable is required")?;
        let api_key = env::var("API_KEY").unwrap_or_default();

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
            base_url,
            model,
            api_key,
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

// ============================================================
// JSON-RPC helpers
// ============================================================

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

// ============================================================
// Agent tools: read, glob, grep
// ============================================================

/// System prompt guiding the LLM to explore the repository using tools.
fn system_prompt(repo_root: &str) -> String {
    format!(
        r#"You are a FastContext repository exploration agent. Your job is to locate
relevant code files and line ranges that answer the user's query.

## Exploration strategy
1. Start BROAD: use `glob` or `grep` to locate candidate files, then `read` specific files.
2. Be thorough: cross-reference symbols, check related files (tests, configs, imports).
3. Batch independent tool calls in the SAME turn whenever possible.
4. NEVER guess file contents — always use tools to verify.
5. If you already know the exact file(s), skip broad searches and read directly.

## Final answer format
When you have sufficient evidence, wrap your output in <final_answer> tags:

<final_answer>
Summary: one-sentence summary of findings.

Files:
- src/auth.rs:15-42  — Authentication middleware setup
- src/routes.rs:88-95  — Route guard calling authenticate()
- src/models/user.rs:200-220 — User session model

Each entry: relative path (from repo root: {repo_root}) + colon + line range + description.
</final_answer>

Rules:
- Return ONLY the <final_answer> block. No preamble, no commentary outside the tags.
- If you cannot find relevant code after thorough search, say so inside <final_answer>.
- Prefer precision over volume: cite only the most relevant 3-10 file ranges."#
    )
}

fn tools_definitions() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "read",
                "description": "Read a file from the repository. Returns content with line numbers, max 200 lines.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Path relative to repository root, e.g. src/main.rs"
                        }
                    },
                    "required": ["file_path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "glob",
                "description": "Find files matching a glob pattern. Returns up to 50 matching paths.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Glob pattern, e.g. **/*.rs, src/**/*.ts, Cargo.toml"
                        }
                    },
                    "required": ["pattern"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "grep",
                "description": "Search file contents with a regex pattern. Returns up to 30 matches as file:line:content.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Regex pattern to search, e.g. fn handle_"
                        },
                        "include": {
                            "type": "string",
                            "description": "Optional file glob filter, e.g. *.rs or src/**/*.rs"
                        }
                    },
                    "required": ["pattern"]
                }
            }
        }
    ])
}

/// Walk a directory recursively, collecting non-hidden file paths.
fn walk_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_symlink() {
            if path.is_file() {
                files.push(path);
            }
            continue;
        }

        if file_type.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Skip hidden dirs and common build artifacts
            if !name.starts_with('.') && name != "node_modules" && name != "target" {
                files.extend(walk_dir(&path)?);
            }
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}

/// Validate a user-supplied path or glob pattern as relative to work_dir.
fn validate_relative_input(input: &str, label: &str) -> Result<PathBuf> {
    let p = PathBuf::from(input);
    if input.trim().is_empty() {
        bail!("{label} cannot be empty");
    }
    if p.is_absolute() {
        bail!("{label} must be relative: {input}");
    }
    for c in p.components() {
        if matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            bail!("{label} must stay under work_dir: {input}");
        }
    }
    Ok(p)
}

/// Sanitize and resolve a file_path relative to work_dir.
/// Rejects paths with `..`, absolute paths, and lexical escapes.
fn sanitize_path(work_dir: &Path, file_path: &str) -> Result<PathBuf> {
    let p = validate_relative_input(file_path, "file_path")?;
    let resolved = work_dir.join(&p);
    if !resolved.starts_with(work_dir) {
        bail!("file_path escapes work_dir: {file_path}");
    }
    Ok(resolved)
}

/// Canonicalize an existing path and ensure symlinks cannot escape work_dir.
fn canonicalize_under_work_dir(work_dir: &Path, path: &Path) -> Result<PathBuf> {
    let root = work_dir
        .canonicalize()
        .with_context(|| format!("cannot canonicalize work_dir: {}", work_dir.display()))?;
    let canonical = path
        .canonicalize()
        .with_context(|| format!("cannot canonicalize path: {}", path.display()))?;

    if !canonical.starts_with(&root) {
        bail!(
            "path escapes work_dir through symlink or canonicalization: {}",
            path.display()
        );
    }

    Ok(canonical)
}

fn relative_display_path(work_dir: &Path, path: &Path) -> String {
    let root = work_dir
        .canonicalize()
        .unwrap_or_else(|_| work_dir.to_path_buf());
    path.strip_prefix(&root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn glob_pattern(work_dir: &Path, pattern: &str) -> Result<String> {
    let rel = validate_relative_input(pattern, "glob pattern")?;
    let full_pattern = work_dir.join(rel);
    full_pattern
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("invalid glob pattern path"))
}

/// Read a file with line numbers, limited to MAX_LINES.
fn tool_read(work_dir: &Path, file_path: &str) -> Result<String> {
    const MAX_LINES: usize = 200;
    const MAX_CHARS: usize = 8000;

    let path = sanitize_path(work_dir, file_path)?;
    let path = canonicalize_under_work_dir(work_dir, &path)?;
    let content = fs::read_to_string(&path)
        .with_context(|| format!("cannot read file: {}", path.display()))?;

    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let shown = lines.iter().take(MAX_LINES);

    let mut out = String::new();
    for (i, line) in shown.enumerate() {
        out.push_str(&format!("{:>6}: {}\n", i + 1, line));
        if out.len() > MAX_CHARS {
            out.truncate(MAX_CHARS);
            out.push_str("... (truncated)\n");
            break;
        }
    }
    if total > MAX_LINES {
        out.push_str(&format!("... ({}/{}) lines shown\n", MAX_LINES, total));
    }

    if out.is_empty() {
        out = "(empty file)\n".to_string();
    }
    Ok(out)
}

/// Find files matching a glob pattern.
fn tool_glob(work_dir: &Path, pattern: &str) -> Result<String> {
    const MAX_RESULTS: usize = 50;

    let pattern_str = glob_pattern(work_dir, pattern)?;

    let mut results: Vec<String> = Vec::new();
    for entry in glob::glob(&pattern_str)? {
        match entry {
            Ok(path) => {
                if path.is_dir() {
                    continue;
                }
                let safe_path = match canonicalize_under_work_dir(work_dir, &path) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                if results.len() >= MAX_RESULTS {
                    break;
                }
                results.push(relative_display_path(work_dir, &safe_path));
            }
            Err(e) => results.push(format!("(error: {e})")),
        }
    }

    if results.is_empty() {
        return Ok("(no files matched)\n".to_string());
    }
    Ok(results.join("\n") + "\n")
}

/// Search file contents with regex, optionally filtered by glob include.
fn tool_grep(work_dir: &Path, pattern: &str, include: Option<&str>) -> Result<String> {
    const MAX_RESULTS: usize = 30;

    let re = Regex::new(pattern).map_err(|e| anyhow!("invalid regex pattern '{pattern}': {e}"))?;

    let files: Vec<PathBuf> = if let Some(glob_pat) = include.filter(|s| !s.is_empty()) {
        let pat_str = glob_pattern(work_dir, glob_pat)?;
        glob::glob(&pat_str)?
            .filter_map(|e| e.ok())
            .filter(|p| p.is_file())
            .collect()
    } else {
        walk_dir(work_dir)?
    };

    let mut results: Vec<String> = Vec::new();
    for file_path in files {
        let safe_path = match canonicalize_under_work_dir(work_dir, &file_path) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let content = match fs::read_to_string(&safe_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let rel = relative_display_path(work_dir, &safe_path);

        for (i, line) in content.lines().enumerate() {
            if results.len() >= MAX_RESULTS {
                break;
            }
            if re.is_match(line) {
                results.push(format!("{}:{}:{}", rel, i + 1, line));
            }
        }
        if results.len() >= MAX_RESULTS {
            break;
        }
    }

    if results.is_empty() {
        return Ok(format!("(no matches for pattern '{pattern}')\n"));
    }
    Ok(results.join("\n") + "\n")
}

/// Dispatch a tool call by name and arguments.
fn execute_tool(work_dir: &Path, name: &str, args: &Value) -> Result<String> {
    match name {
        "read" => {
            let file_path = args["file_path"]
                .as_str()
                .ok_or_else(|| anyhow!("read: missing file_path"))?;
            tool_read(work_dir, file_path)
        }
        "glob" => {
            let pattern = args["pattern"]
                .as_str()
                .ok_or_else(|| anyhow!("glob: missing pattern"))?;
            tool_glob(work_dir, pattern)
        }
        "grep" => {
            let pattern = args["pattern"]
                .as_str()
                .ok_or_else(|| anyhow!("grep: missing pattern"))?;
            let include = args["include"].as_str();
            tool_grep(work_dir, pattern, include)
        }
        other => bail!("unknown tool: {other}"),
    }
}

fn collect_tool_evidence(evidence: &mut Vec<String>, name: &str, raw_args: &str, result: &str) {
    const MAX_EVIDENCE: usize = 40;
    const MAX_LINES_PER_TOOL: usize = 8;
    const MAX_LINE_CHARS: usize = 320;

    if evidence.len() >= MAX_EVIDENCE {
        return;
    }

    let mut added = 0;
    for line in result.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("Error:")
            || trimmed.starts_with("(no ")
            || trimmed.starts_with("... (")
        {
            continue;
        }

        let mut item = trimmed.to_string();
        if item.len() > MAX_LINE_CHARS {
            item.truncate(MAX_LINE_CHARS);
            item.push_str("...");
        }

        evidence.push(format!("- {name} {raw_args}: {item}"));
        added += 1;
        if added >= MAX_LINES_PER_TOOL || evidence.len() >= MAX_EVIDENCE {
            break;
        }
    }
}

fn partial_final_answer(reason: &str, evidence: &[String]) -> String {
    let mut out = String::from("<final_answer>\n");
    out.push_str(&format!(
        "Summary: Partial answer generated because {reason}.\n\n"
    ));
    out.push_str("Files:\n");

    if evidence.is_empty() {
        out.push_str("- No usable file evidence was collected before stopping.\n");
    } else {
        for item in evidence.iter().take(20) {
            out.push_str(item);
            out.push('\n');
        }
    }

    out.push_str("</final_answer>");
    out
}

// ============================================================
// LLM client
// ============================================================

/// Send a chat completion request to the LLM endpoint.
fn llm_chat(
    base_url: &str,
    model: &str,
    api_key: &str,
    messages: &[Value],
    tools_def: &Value,
) -> Result<Value> {
    let body = json!({
        "model": model,
        "messages": messages,
        "tools": tools_def,
        "tool_choice": "auto",
        "max_tokens": 4096,
        "temperature": 0.1,
    });

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let resp = ureq::post(&url)
        .set("Content-Type", "application/json")
        .set("Authorization", &format!("Bearer {}", api_key))
        .send_json(&body)
        .map_err(|e| anyhow!("LLM API request failed: {e} (url: {url})"))?;

    let json: Value = resp
        .into_json()
        .map_err(|e| anyhow!("failed to parse LLM response: {e}"))?;

    // Check for API-level errors
    if let Some(err) = json.get("error") {
        bail!(
            "LLM API error: {}",
            err["message"].as_str().unwrap_or("unknown")
        );
    }

    Ok(json)
}

// ============================================================
// Agent loop
// ============================================================

/// Run the exploration agent loop: send messages to the LLM, execute tool
/// calls, and return the final answer.
fn run_explorer(config: &Config, args: ExploreArgs) -> Result<String> {
    let query = args.query.trim().to_string();
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
    let timeout_secs = args
        .timeout_secs
        .unwrap_or(config.default_timeout_secs)
        .clamp(10, 1800);

    // Effective endpoint settings (per-request overrides)
    let base_url = args.base_url.unwrap_or_else(|| config.base_url.clone());
    let model = args.model.unwrap_or_else(|| config.model.clone());
    let api_key = args.api_key.unwrap_or_else(|| config.api_key.clone());

    let tools_def = tools_definitions();
    let sys_prompt = system_prompt(&work_dir.display().to_string());

    let mut messages: Vec<Value> = vec![
        json!({"role": "system", "content": sys_prompt}),
        json!({"role": "user", "content": query}),
    ];

    let start = std::time::Instant::now();
    let mut seen_tool_calls: HashSet<String> = HashSet::new();
    let mut repeated_tool_calls = 0usize;
    let mut evidence: Vec<String> = Vec::new();

    for _turn in 0..max_turns {
        if start.elapsed().as_secs() > timeout_secs {
            if evidence.is_empty() {
                bail!("exploration timed out after {timeout_secs}s");
            }
            return Ok(partial_final_answer(
                &format!("exploration timed out after {timeout_secs}s"),
                &evidence,
            ));
        }

        let resp = llm_chat(&base_url, &model, &api_key, &messages, &tools_def)?;

        let choices = resp["choices"]
            .as_array()
            .ok_or_else(|| anyhow!("LLM response missing choices array"))?;

        if choices.is_empty() {
            bail!("LLM returned empty choices");
        }

        let choice = &choices[0];
        let finish_reason = choice["finish_reason"].as_str().unwrap_or("stop");
        let msg = &choice["message"];

        match finish_reason {
            "stop" => {
                let content = msg["content"].as_str().unwrap_or("");
                return Ok(content.to_string());
            }
            "tool_calls" => {
                let tool_calls = msg["tool_calls"].as_array().cloned().unwrap_or_default();
                if tool_calls.is_empty() {
                    if evidence.is_empty() {
                        bail!("LLM requested tool calls but returned none");
                    }
                    return Ok(partial_final_answer(
                        "the LLM requested tool calls but returned none",
                        &evidence,
                    ));
                }

                // Push assistant message with tool_calls
                let mut assistant_msg = json!({
                    "role": "assistant",
                    "content": msg["content"],
                    "tool_calls": tool_calls,
                });
                // Avoid null content — OpenAI expects null, not absent
                if assistant_msg["content"].is_null() {
                    assistant_msg["content"] = Value::Null;
                }
                messages.push(assistant_msg);

                for tc in &tool_calls {
                    let id = tc["id"].as_str().unwrap_or("call_unknown");
                    let func = &tc["function"];
                    let name = func["name"].as_str().unwrap_or("unknown");
                    let raw_args = func["arguments"].as_str().unwrap_or("{}");
                    let signature = format!("{name}:{raw_args}");

                    let args_val: Value =
                        serde_json::from_str(raw_args).unwrap_or_else(|_| json!({}));

                    let result = if !seen_tool_calls.insert(signature) {
                        repeated_tool_calls += 1;
                        format!("Error: repeated tool call skipped: {name} {raw_args}")
                    } else {
                        let result = execute_tool(&work_dir, name, &args_val)
                            .unwrap_or_else(|e| format!("Error: {e}"));
                        collect_tool_evidence(&mut evidence, name, raw_args, &result);
                        result
                    };

                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": id,
                        "content": result,
                    }));
                }

                if repeated_tool_calls >= 2 && !evidence.is_empty() {
                    return Ok(partial_final_answer(
                        "the LLM repeated the same tool calls without producing a final answer",
                        &evidence,
                    ));
                }
            }
            other => {
                bail!("LLM stopped unexpectedly (finish_reason: {other})");
            }
        }
    }

    if evidence.is_empty() {
        bail!("reached maximum exploration turns ({max_turns}) without final answer");
    }

    Ok(partial_final_answer(
        &format!("the LLM reached maximum exploration turns ({max_turns}) without a final answer"),
        &evidence,
    ))
}

// ============================================================
// MCP handlers
// ============================================================

fn tools_list_result() -> Value {
    json!({
        "tools": [
            {
                "name": "fastcontext_explore",
                "description": "Explore a repository using FastContext-1.0-4B-RL. Uses an LLM agent loop with Read/Glob/Grep tools to find relevant code. Read-only; returns file paths and line ranges.",
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
                            "default": 6,
                            "description": "Maximum LLM tool-call iterations."
                        },
                        "timeout_secs": {
                            "type": "integer",
                            "minimum": 10,
                            "maximum": 1800,
                            "default": 300
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
                "description": "Check the MCP server configuration and LLM endpoint availability. Read-only diagnostic tool.",
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

/// Build a server status report string.
fn server_status_text(config: &Config) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "server:     {} v{}",
        SERVER_NAME,
        env!("CARGO_PKG_VERSION")
    ));
    lines.push(format!("base_url:   {}", config.base_url));
    lines.push(format!("model:      {}", config.model));
    lines.push(format!(
        "api_key:    {}",
        if config.api_key.is_empty() {
            "(not set) — OK for local servers".to_string()
        } else {
            "✓ (set)".to_string()
        }
    ));
    lines.push(format!("work_dir:   {}", config.default_work_dir.display()));
    lines.push(format!("allowed_root: {}", config.allowed_root.display()));
    lines.push(format!("max_turns:  {}", config.default_max_turns));
    lines.push(format!("timeout:    {}s", config.default_timeout_secs));
    lines.join("\n")
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

            match run_explorer(config, args) {
                Ok(text) => {
                    json!({"content": [{"type": "text", "text": text}], "isError": false})
                }
                Err(err) => {
                    let msg = err.to_string();
                    // Enrich connection errors
                    let enriched = if msg.contains("LLM API request failed")
                        || msg.contains("Connection refused")
                    {
                        format!(
                            "{}\n\nHint: Make sure your FastContext model server is running.\n\
                             Start it with: scripts/run_llama_fastcontext_rl.ps1\n\
                             Or set BASE_URL to point to your running server.",
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

// ============================================================
// Entrypoint
// ============================================================

fn main() -> Result<()> {
    let config = Config::from_env()?;
    let stdin = io::stdin();

    // Startup diagnostics
    eprintln!("{SERVER_NAME} v{}", env!("CARGO_PKG_VERSION"));
    eprintln!("  base_url:   {}", config.base_url);
    eprintln!("  model:      {}", config.model);
    eprintln!("  work_dir:   {}", config.default_work_dir.display());
    eprintln!("  allowed:    {}", config.allowed_root.display());
    eprintln!("  max_turns:  {}", config.default_max_turns);
    eprintln!("  timeout:    {}s", config.default_timeout_secs);

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

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            base_url: "http://127.0.0.1:30000/v1".to_string(),
            model: "test-model".to_string(),
            api_key: String::new(),
            default_work_dir: PathBuf::from("."),
            allowed_root: PathBuf::from("."),
            default_max_turns: 6,
            default_timeout_secs: 300,
        }
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let unique = format!(
            "fastcontext_mcp_{}_{}_{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = env::temp_dir().join(unique);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn create_file_symlink(src: &Path, dst: &Path) -> io::Result<()> {
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(src, dst)
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(src, dst)
        }
    }

    // -- Config::from_env relies on env vars; tested via manual construction

    // -- Config::resolve_work_dir --

    #[test]
    fn test_resolve_work_dir_default() {
        let config = test_config();
        let result = config.resolve_work_dir(None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_work_dir_outside_allowed() {
        let config = test_config();
        let result = config.resolve_work_dir(Some("C:\\Windows\\System32"));
        assert!(result.is_err());
    }

    // -- sanitize_path --

    #[test]
    fn test_sanitize_path_valid() {
        let wd = PathBuf::from("/repo");
        let result = sanitize_path(&wd, "src/main.rs");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), wd.join("src/main.rs"));
    }

    #[test]
    fn test_sanitize_path_rejects_absolute() {
        let wd = PathBuf::from("/repo");
        assert!(sanitize_path(&wd, "/etc/passwd").is_err());
    }

    #[test]
    fn test_sanitize_path_rejects_parent() {
        let wd = PathBuf::from("/repo");
        assert!(sanitize_path(&wd, "../outside").is_err());
    }

    #[test]
    fn test_sanitize_path_rejects_deep_parent() {
        let wd = PathBuf::from("/repo");
        assert!(sanitize_path(&wd, "a/b/../../../../etc/passwd").is_err());
    }

    #[test]
    fn test_tool_read_rejects_symlink_escape() {
        let root = temp_test_dir("root");
        let outside = temp_test_dir("outside");
        let outside_file = outside.join("secret.txt");
        fs::write(&outside_file, "secret").unwrap();
        let link_path = root.join("link.txt");

        if create_file_symlink(&outside_file, &link_path).is_err() {
            let _ = fs::remove_dir_all(&root);
            let _ = fs::remove_dir_all(&outside);
            return;
        }

        let result = tool_read(&root, "link.txt");
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
        assert!(result.is_err());
    }

    // -- tool_read --

    #[test]
    fn test_tool_read_nonexistent_file() {
        let wd = PathBuf::from(".");
        let result = tool_read(&wd, "this_file_does_not_exist_42xyz.rs");
        assert!(result.is_err());
    }

    // -- tool_glob --

    #[test]
    fn test_tool_glob_no_match() {
        let wd = PathBuf::from(".");
        let result = tool_glob(&wd, "**/*.nonexistent_xyz").unwrap();
        assert!(result.contains("no files matched"));
    }

    #[test]
    fn test_tool_glob_finds_cargo_toml() {
        let wd = PathBuf::from(".");
        let result = tool_glob(&wd, "Cargo.toml").unwrap();
        assert!(result.contains("Cargo.toml"));
    }

    #[test]
    fn test_tool_glob_rejects_parent_pattern() {
        let wd = PathBuf::from(".");
        let result = tool_glob(&wd, "../*.rs");
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_glob_rejects_absolute_pattern() {
        let wd = PathBuf::from(".");
        let absolute = env::temp_dir().join("*.rs");
        let result = tool_glob(&wd, &absolute.display().to_string());
        assert!(result.is_err());
    }

    // -- tool_grep --

    #[test]
    fn test_tool_grep_no_match() {
        let wd = PathBuf::from(".");
        let result = tool_grep(&wd, "nothing", Some("*.nonexistent_ext_xyz")).unwrap();
        assert!(result.contains("no matches"));
    }

    #[test]
    fn test_tool_grep_finds_existing_content() {
        let wd = PathBuf::from(".");
        // This file should contain "fastcontext_explore"
        let result = tool_grep(&wd, "fastcontext_explore", Some("*.rs")).unwrap();
        assert!(result.contains("fastcontext_explore"));
    }

    #[test]
    fn test_tool_grep_rejects_parent_include() {
        let wd = PathBuf::from(".");
        let result = tool_grep(&wd, "anything", Some("../*.rs"));
        assert!(result.is_err());
    }

    #[test]
    fn test_partial_final_answer_uses_final_answer_tags() {
        let evidence = vec!["- read {\"file_path\":\"src/main.rs\"}: 1: fn main()".to_string()];
        let answer = partial_final_answer("the model reached max turns", &evidence);
        assert!(answer.starts_with("<final_answer>"));
        assert!(answer.contains("Partial answer"));
        assert!(answer.contains("src/main.rs"));
        assert!(answer.ends_with("</final_answer>"));
    }

    // -- execute_tool --

    #[test]
    fn test_execute_tool_unknown() {
        let wd = PathBuf::from(".");
        let result = execute_tool(&wd, "nonexistent_tool", &json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_tool_read_missing_arg() {
        let wd = PathBuf::from(".");
        let result = execute_tool(&wd, "read", &json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_tool_glob_missing_arg() {
        let wd = PathBuf::from(".");
        let result = execute_tool(&wd, "glob", &json!({}));
        assert!(result.is_err());
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
            "timeout_secs": 120,
            "base_url": "http://localhost:8080/v1",
            "model": "my-model",
            "api_key": "sk-test"
        });
        let args: ExploreArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.query, "find routes");
        assert_eq!(args.work_dir.unwrap(), "/repo");
        assert_eq!(args.max_turns.unwrap(), 10);
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
    }

    #[test]
    fn test_explore_args_rejects_empty_query() {
        let json = json!({"query": ""});
        let args: ExploreArgs = serde_json::from_value(json).unwrap();
        assert!(args.query.is_empty());
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

    // -- server_status_text --

    #[test]
    fn test_server_status_text_includes_server_name() {
        let cfg = test_config();
        let report = server_status_text(&cfg);
        assert!(report.contains("fastcontext-mcp-rust"));
        assert!(report.contains("base_url"));
        assert!(report.contains("model"));
    }

    #[test]
    fn test_server_status_text_contains_config_values() {
        let cfg = test_config();
        let report = server_status_text(&cfg);
        assert!(report.contains("http://127.0.0.1:30000/v1"));
        assert!(report.contains("test-model"));
        assert!(report.contains("max_turns"));
        assert!(report.contains("timeout"));
    }

    // -- system_prompt -- (no assert, just ensure no panic)
    #[test]
    fn test_system_prompt_contains_rules() {
        let prompt = system_prompt("/repo");
        assert!(prompt.contains("read"));
        assert!(prompt.contains("glob"));
        assert!(prompt.contains("grep"));
        assert!(prompt.contains("/repo"));
    }

    // -- tools_definitions --
    #[test]
    fn test_tools_definitions_has_three_tools() {
        let defs = tools_definitions();
        let arr = defs.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        let names: Vec<&str> = arr
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"read"));
        assert!(names.contains(&"glob"));
        assert!(names.contains(&"grep"));
    }
}
