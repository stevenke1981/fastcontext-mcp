# fastcontext-mcp-rust — Agent Guide

This file is for **AI coding agents** (OpenCode, Claude Code, etc.) working on this project.

## Project Overview

`fastcontext-mcp-rust` is a Rust MCP (Model Context Protocol) stdio server with a
**built-in agent loop**. It communicates directly with a FastContext LLM via OpenAI
Chat Completions API and provides Read/Glob/Grep tools for repository exploration.

```
OpenCode / MCP client
  -> fastcontext-mcp-rust  (this project, agent loop + tools)
  -> FastContext-1.0-4B-RL (LLM via llama.cpp or SGLang)
```

## Repository Layout

```
.cargo/config.toml          Release LTO & native CPU optimizations
.github/workflows/ci.yml    CI: build, clippy, test, fmt
.opencode/                  OpenCode plans & internal state
examples/
  opencode.jsonc            Example MCP client config
scripts/
  run_llama_fastcontext_rl.ps1/sh    llama.cpp launcher
  run_sglang_fastcontext_rl.ps1/sh   SGLang launcher
src/
  main.rs                   Full MCP server (~950 lines, agent loop + tools)
  (tests in #[cfg(test)] mod at bottom of main.rs)
install.ps1 / install.sh    Install to ~/.cargo/bin
uninstall.ps1 / uninstall.sh
README.md / README.zh-TW.md   User docs
AGENTS.md / AGENTS.zh-TW.md   Agent docs (this file)
Cargo.toml                   Dependencies: tokio, serde, anyhow
lessons.md                   RSI lessons
```

## Code Conventions

- **Single file:** All logic in `src/main.rs`. Keep it focused; extract modules only when it exceeds ~1200 lines.
- **Error handling:** Use `anyhow::Result` / `bail!` / `Context`. No unwrap in production code (tests may use `.unwrap()`).
- **MCP protocol:** JSON-RPC 2.0 over stdin/stdout. Lines are newline-delimited JSON.
- **Config:** Read from environment variables via `Config::from_env()`. Tool arguments can override per-request.

## Build & Test Commands

```bash
cargo check                    # Fast compilation check
cargo clippy --all-targets --all-features  # Lint (must pass clean)
cargo test                     # Run unit tests (currently 31)
cargo fmt --check              # Format check
cargo build --release          # Release build with LTO
```

## Key Architecture Decisions

### Built-in Agent Loop:

`fastcontext-mcp-rust` implements its own exploration agent loop in Rust —
**no external CLI dependency required**. The flow:

1. Receive query via `fastcontext_explore` tool
2. Build system prompt + tool definitions for the LLM
3. Call the FastContext model via OpenAI Chat Completions API (`POST /v1/chat/completions`)
4. LLM returns `tool_calls` → execute Read/Glob/Grep on the filesystem → feed results back
5. LLM returns `stop` (final answer) → return evidence to caller
6. Configurable max turns (default 6) and timeout (default 300s)

### Tools provided to the LLM:

1. **`read`** — Read a file with line numbers, max 200 lines.
2. **`glob`** — Find files matching a glob pattern, max 50 results.
3. **`grep`** — Search file contents with regex, max 30 matches.

### MCP tools exposed to OpenCode:

1. **`fastcontext_explore`** — Main tool. Triggers the built-in agent loop.
   Args: `query` (required), `work_dir`, `max_turns`, `timeout_secs`, `base_url`, `model`, `api_key`.

2. **`fastcontext_status`** — Read-only diagnostic tool. Returns config and endpoint status.

### Safety:

- `work_dir` is validated against `FASTCONTEXT_ALLOWED_ROOT` (canonical path check).
- All file paths passed to Read/Glob/Grep are sanitized: `../`, absolute paths, and
  symlink escapes are rejected.
- Read output is capped at 8000 characters; Glob at 50 results; Grep at 30 matches.
- Agent loop has configurable timeout (default 300s).
- The server is intentionally read-only — no shell execution, file writing, or code modification.

### Startup diagnostics:

On startup, the server prints:
- `BASE_URL` and `MODEL` being used
- Work directory and allowed root
- Max turns and timeout configuration

## Testing

All tests are in `src/main.rs` under `#[cfg(test)] mod tests`.
Currently 35 tests covering:
- Path validation (relative, absolute, parent-dir, root)
- Tool execution (read, glob, grep)
- Tool definitions schema
- JSON-RPC response format
- ExploreArgs deserialization (including base_url/model/api_key)
- Request handlers (initialize, tools/list, ping, unknown, notification)
- Config::resolve_work_dir
- server_status_text output
- system_prompt validation

## When Modifying

1. Update `tools_list_result()` when adding/changing tool input schemas.
2. Update `handle_tools_call()` dispatch for new tools.
3. Update startup diagnostics in `main()` if adding new env vars.
4. Add tests for new functionality.
5. Update `AGENTS.md` for structural changes.
6. Run full verification: `cargo fmt --check && cargo clippy --all-targets --all-features && cargo test`
