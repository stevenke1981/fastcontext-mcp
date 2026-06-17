# fastcontext-mcp-rust

[![CI](https://github.com/stevenke1981/fastcontext-mcp/actions/workflows/ci.yml/badge.svg)](https://github.com/stevenke1981/fastcontext-mcp/actions/workflows/ci.yml)
[![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange)](rust-toolchain.toml)
![GitHub last commit](https://img.shields.io/github/last-commit/stevenke1981/fastcontext-mcp)

以 Rust 撰寫的本地 MCP stdio 伺服器，內建 **LLM agent loop**。它直接與本地
FastContext 模型（透過 OpenAI Chat Completions API）通訊，使用 Read/Glob/Grep
工具進行倉庫探索 ── 不需外部 CLI 依賴。

## 目錄

- [架構](#架構)
- [快速開始](#快速開始)
  - [1. 在本機執行模型](#1-在本機執行模型)
  - [2. 安裝 FastContext CLI](#2-安裝-fastcontext-cli)
  - [3. 建置 MCP 伺服器](#3-建置-mcp-伺服器)
  - [4. OpenCode 配置](#4-opencode-配置)
- [工具](#工具)
  - [`fastcontext_explore`](#fastcontext_explore)
  - [`fastcontext_status`](#fastcontext_status)
- [環境變數](#環境變數)
- [開發](#開發)
- [安全注意事項](#安全注意事項)

## 架構

```text
OpenCode / MCP client
  -> fastcontext-mcp-rust over stdio (agent loop + read/glob/grep tools)
  -> http://127.0.0.1:30000/v1/chat/completions
  -> microsoft/FastContext-1.0-4B-RL
       Option A: SGLang/vLLM (BF16, full precision)
       Option B: llama.cpp   (GGUF Q4_K_M, 2.5 GB, no Python needed)
```

## 快速開始

### 1. 在本機執行模型

**SGLang**（完整精度）：

```bash
pip install "sglang[all]"
./scripts/run_sglang_fastcontext_rl.sh
```

Windows PowerShell：

```powershell
pip install "sglang[all]"
./scripts/run_sglang_fastcontext_rl.ps1
```

**llama.cpp**（GGUF，不需 Python 執行環境）：

安裝 [llama.cpp](https://github.com/ggml-org/llama.cpp)（從原始碼建置或使用發佈二進位檔），然後：

```bash
./scripts/run_llama_fastcontext_rl.sh
```

Windows PowerShell：

```powershell
./scripts/run_llama_fastcontext_rl.ps1
```

腳本會在首次執行時自動從 HuggingFace 下載 GGUF 模型
（`mitkox/FastContext-1.0-4B-RL-Q4_K_M-GGUF`，約 2.5 GB）。
若記憶體不足，可使用 `--ctx-size 65536` 減少上下文長度。

### 2. 安裝 FastContext CLI

```bash
git clone https://github.com/microsoft/fastcontext.git
cd fastcontext
uv tool install .
```

驗證：

```bash
fastcontext --query "Locate request validation logic" --citation
```

### 3. 建置 MCP 伺服器

```bash
cargo build --release
```

二進位檔位於 `target/release/fastcontext-mcp-rust.exe`。

使用安裝腳本可將伺服器安裝到系統 PATH：

```bash
# Windows PowerShell
./install.ps1

# Linux / macOS
./install.sh
```

解除安裝：

```bash
# Windows PowerShell
./uninstall.ps1

# Linux / macOS
./uninstall.sh
```

### 4. OpenCode 配置

將 `examples/opencode.jsonc` 複製到您的 OpenCode 配置中，
並根據您的環境調整路徑。

## 工具

### `fastcontext_explore`

使用 Microsoft FastContext CLI 探索倉庫。唯讀操作；回傳精簡的檔案路徑與行號範圍。

引數：

```json
{
  "query": "尋找認證中介軟體的實作位置",
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

選擇性的 `base_url`、`model`、`api_key` 欄位可覆蓋對應的環境變數，
讓您在不重啟 MCP 伺服器的情況下切換端點。

### `fastcontext_status`

診斷工具——回傳伺服器配置、`fastcontext` 二進位檔可用性、環境變數狀態與預設設定。

```json
{
  "name": "fastcontext_status",
  "arguments": {}
}
```

回應範例：

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

## 環境變數

| 變數 | 預設值 | 說明 |
|------|--------|------|
| `FASTCONTEXT_BIN` | `fastcontext` | FastContext CLI 二進位檔路徑 |
| `FASTCONTEXT_WORK_DIR` | 目前目錄 | 預設倉庫工作目錄 |
| `FASTCONTEXT_ALLOWED_ROOT` | 同 work dir | `work_dir` 驗證的根目錄 |
| `FASTCONTEXT_MAX_TURNS` | `6` | 預設最大探索回合數 |
| `FASTCONTEXT_TIMEOUT_SECS` | `300` | 預設指令逾時秒數 |
| `BASE_URL` | _（必填）_ | OpenAI 相容端點 URL，例如 `http://127.0.0.1:30000/v1` |
| `MODEL` | _（必填）_ | 模型名稱，例如 `microsoft/FastContext-1.0-4B-RL` |
| `API_KEY` | `dummy` | 端點的 API 金鑰 |

工具引數 `base_url`、`model`、`api_key` 可覆蓋對應的環境變數。

## 開發

前置需求：[Rust](https://rustup.rs/)（stable）、`fastcontext` CLI。

```bash
# 檢查程式碼
cargo check

# Lint
cargo clippy --all-targets --all-features

# 執行測試
cargo test

# 格式化
cargo fmt --check
```

CI 會在推送 / PR 時透過 GitHub Actions 自動執行這些檢查（參見 [ci.yml](.github/workflows/ci.yml)）。

## 安全注意事項

此封裝器刻意設計為唯讀。它僅暴露 FastContext 探索功能，
不暴露 shell 執行、檔案寫入、git、cargo 或任意指令。
`work_dir` 必須在 `FASTCONTEXT_ALLOWED_ROOT` 內；
`trajectory_file` 必須是倉庫下的相對路徑。
