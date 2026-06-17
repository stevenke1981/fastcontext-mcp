# fastcontext-mcp-rust

Local MCP stdio server written in Rust. It wraps the upstream `fastcontext` CLI and points it at a local OpenAI-compatible model server running `microsoft/FastContext-1.0-4B-RL`.

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

## 1. Run the model locally

SGLang:

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

## 2. Install FastContext CLI

```bash
git clone https://github.com/microsoft/fastcontext.git
cd fastcontext
uv tool install .
```

Check:

```bash
fastcontext --query "Locate request validation logic" --citation
```

## 3. Build MCP server

```bash
cargo build --release
```

## 4. OpenCode config

Copy `examples/opencode.jsonc` into your OpenCode config and change paths.

## Tool

`fastcontext_explore`

Arguments:

```json
{
  "query": "Find where authentication middleware is implemented",
  "work_dir": "D:/your-repo",
  "max_turns": 6,
  "citation": true,
  "trajectory_file": ".fastcontext/trajectory.jsonl"
}
```

## Environment variables

- `FASTCONTEXT_BIN`: default `fastcontext`
- `FASTCONTEXT_WORK_DIR`: default current directory
- `FASTCONTEXT_ALLOWED_ROOT`: default same as work dir
- `FASTCONTEXT_MAX_TURNS`: default `6`
- `FASTCONTEXT_TIMEOUT_SECS`: default `300`
- `BASE_URL`: default must be supplied to FastContext CLI, e.g. `http://127.0.0.1:30000/v1`
- `MODEL`: `microsoft/FastContext-1.0-4B-RL`
- `API_KEY`: can be `dummy` for local SGLang/vLLM unless you configured auth

## Security notes

This wrapper is intentionally read-only. It only exposes FastContext exploration and does not expose shell execution, file writing, git, cargo, or arbitrary commands. `work_dir` must stay inside `FASTCONTEXT_ALLOWED_ROOT`; `trajectory_file` must be a relative path under the repository.
