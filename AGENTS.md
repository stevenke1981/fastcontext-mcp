# fastcontext-mcp-rust — Agent Guide

This file is for **AI coding agents** (OpenCode, Claude Code, etc.) working on this project.

## Project Overview

`fastcontext-mcp-rust` is a Rust MCP (Model Context Protocol) stdio server. It wraps the
Microsoft FastContext CLI and exposes a `fastcontext_explore` tool for repository exploration
via natural language queries.

```
OpenCode / MCP client
  -> fastcontext-mcp-rust  (this project)
  -> fastcontext CLI       (external Python tool)
  -> FastContext-1.0-4B-RL (LLM via SGLang or llama.cpp)
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
  main.rs                   Full MCP server (~870 lines)
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

### Tools provided:

1. **`fastcontext_explore`** — Main tool. Spawns `fastcontext` CLI as a subprocess with piped I/O.
   Args: `query` (required), `work_dir`, `max_turns`, `citation`, `trajectory_file`,
   `timeout_secs`, `verbose`, `base_url`, `model`, `api_key`.

2. **`fastcontext_status`** — Read-only diagnostic tool. Returns config, binary status,
   environment variable state.

### Safety:

- `work_dir` is validated against `FASTCONTEXT_ALLOWED_ROOT` (canonical path check).
- `trajectory_file` must be relative; absolute and parent-dir paths are rejected.
- FastContext spawn has a configurable timeout (default 300s).
- The server is intentionally read-only — no shell execution, file writing, or code modification.

### Startup diagnostics:

On startup, the server checks:
- Whether the `fastcontext` binary is on PATH
- Whether `BASE_URL` and `MODEL` env vars are set
- Prints warnings for missing dependencies

## Testing

All tests are in `src/main.rs` under `#[cfg(test)] mod tests`.
Currently 31 tests covering:
- Path validation (relative, absolute, parent-dir, root)
- Error truncation
- Tool list schema
- JSON-RPC response format
- ExploreArgs deserialization (including base_url/model/api_key)
- Request handlers (initialize, tools/list, ping, unknown, notification)
- Config::resolve_work_dir
- server_status_text output
- check_fastcontext_binary

## When Modifying

1. Update `tools_list_result()` when adding/changing tool input schemas.
2. Update `handle_tools_call()` dispatch for new tools.
3. Update startup diagnostics in `main()` if adding new env vars.
4. Add tests for new functionality.
5. Update `AGENTS.md` for structural changes.
6. Run full verification: `cargo fmt --check && cargo clippy --all-targets --all-features && cargo test`
