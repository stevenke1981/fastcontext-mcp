# fastcontext-mcp-rust

[![CI](https://github.com/stevenke1981/fastcontext-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/stevenke1981/fastcontext-mcp/actions/workflows/ci.yml)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange)](rust-toolchain.toml)
![GitHub last commit](https://img.shields.io/github/last-commit/stevenke1981/fastcontext-mcp)

Local MCP stdio server written in Rust. It wraps the upstream `fastcontext` CLI and points it at a local OpenAI-compatible model server running `microsoft/FastContext-1.0-4B-RL`.

## Table of Contents

- [Architecture](#architecture)
- [Quick Start](#quick-start)
  - [1. Run the model locally](#1-run-the-model-locally)
  - [2. Install FastContext CLI](#2-install-fastcontext-cli)
  - [3. Build MCP server](#3-build-mcp-server)
  - [4. OpenCode config](#4-opencode-config)
- [Tools](#tools)
  - [`fastcontext_explore`](#fastcontext_explore)
  - [`fastcontext_status`](#fastcontext_status)
- [Environment variables](#environment-variables)
- [Development](#development)
- [Security notes](#security-notes)

## Architecture

```text
OpenCode / MCP client
  -> fastcontext-mcp-rust over stdio
  -> fastcontext CLI
  -> http://127.0.0.1:30000/v1/chat/completions
  -> microsoft/FastContext-1.0-4B-RL
       Option A: SGLang/vLLM (BF16, full precision)
       Option B: llama.cpp   (GGUF Q4_K_M, 2.5 GB, no Python needed)
```

## Quick Start

### 1. Run the model locally

**SGLang** (full precision):

```bash
pip install "sglang[all]"
./scripts/run_sglang_fastcontext_rl.sh
```

Windows PowerShell:

```powershell
pip install "sglang[all]"
./scripts/run_sglang_fastcontext_rl.ps1
```

**llama.cpp** (GGUF, no Python runtime required):

Install [llama.cpp](https://github.com/ggml-org/llama.cpp) (build from source or use a release binary), then:

```bash
./scripts/run_llama_fastcontext_rl.sh
```

Windows PowerShell:

```powershell
./scripts/run_llama_fastcontext_rl.ps1
```

The script auto-downloads the GGUF model (`mitkox/FastContext-1.0-4B-RL-Q4_K_M-GGUF`, ~2.5 GB) from HuggingFace on first run. Reduce context with `--ctx-size 65536` if memory constrained.

### 2. Install FastContext CLI

```bash
git clone https://github.com/microsoft/fastcontext.git
cd fastcontext
uv tool install .
```

Check:

```bash
fastcontext --query "Locate request validation logic" --citation
```

### 3. Build MCP server

```bash
cargo build --release
```

The binary is at `target/release/fastcontext-mcp-rust.exe`.

### 4. OpenCode config

Copy `examples/opencode.jsonc` into your OpenCode config and adjust paths for your setup.

## Tools

### `fastcontext_explore`

Explore a repository using Microsoft FastContext CLI. Read-only; returns compact file paths and line ranges.

Arguments:

```json
{
  "query": "Find where authentication middleware is implemented",
  "work_dir": "D:/your-repo",
  "max_turns": 6,
  "citation": true,
  "trajectory_file": ".fastcontext/trajectory.jsonl",
  "timeout_secs": 300,
  "verbose": false,
  "base_url": "http://127.0.0.1:30000/v1",
  "model": "microsoft/FastContext-1.0-4B-RL",
  "api_key": "dummy"
}
```

The optional `base_url`, `model`, and `api_key` fields override the corresponding environment variables for that single request — useful for switching between endpoints without restarting the MCP server.

### `fastcontext_status`

Diagnostic tool — returns server configuration, `fastcontext` binary availability, environment variable status, and default settings.

```json
{
  "name": "fastcontext_status",
  "arguments": {}
}
```

Response example:

```
server:     fastcontext-mcp-rust v0.1.0
binary:     fastcontext ✓
work_dir:   D:\repo
allowed_root: D:\repo
max_turns:  6
timeout:    300s
BASE_URL:   http://127.0.0.1:30000/v1
MODEL:      microsoft/FastContext-1.0-4B-RL
API_KEY:    ✓ (set)
```

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `FASTCONTEXT_BIN` | `fastcontext` | Path to the FastContext CLI binary |
| `FASTCONTEXT_WORK_DIR` | current directory | Default repository work directory |
| `FASTCONTEXT_ALLOWED_ROOT` | same as work dir | Directory root for `work_dir` validation |
| `FASTCONTEXT_MAX_TURNS` | `6` | Default max exploration turns |
| `FASTCONTEXT_TIMEOUT_SECS` | `300` | Default command timeout in seconds |
| `BASE_URL` | _(required)_ | OpenAI-compatible endpoint URL, e.g. `http://127.0.0.1:30000/v1` |
| `MODEL` | _(required)_ | Model name, e.g. `microsoft/FastContext-1.0-4B-RL` |
| `API_KEY` | `dummy` | API key for the endpoint |

The tool arguments `base_url`, `model`, `api_key` override the corresponding environment variables per-request.

## Development

Prerequisites: [Rust](https://rustup.rs/) (stable), `fastcontext` CLI.

```bash
# Check code
cargo check

# Lint
cargo clippy --all-targets --all-features

# Run tests
cargo test

# Format
cargo fmt --check
```

CI runs these checks automatically on push/PR via GitHub Actions (see [ci.yml](.github/workflows/ci.yml)).

## Security notes

This wrapper is intentionally read-only. It only exposes FastContext exploration and does not expose shell execution, file writing, git, cargo, or arbitrary commands. `work_dir` must stay inside `FASTCONTEXT_ALLOWED_ROOT`; `trajectory_file` must be a relative path under the repository.
