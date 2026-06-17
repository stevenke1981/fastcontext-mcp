# fastcontext-mcp-rust — Agent 指南

本文件供 **AI 程式碼代理**（OpenCode、Claude Code 等）在處理此專案時使用。

## 專案概述

`fastcontext-mcp-rust` 是一個 Rust 實作的 MCP（Model Context Protocol）stdio 伺服器。
它封裝了 Microsoft FastContext CLI，並透過 `fastcontext_explore` 工具以自然語言查詢進行
倉庫探索。

```
OpenCode / MCP client
  -> fastcontext-mcp-rust  (本專案)
  -> fastcontext CLI       (外部 Python 工具)
  -> FastContext-1.0-4B-RL (透過 SGLang 或 llama.cpp 執行的 LLM)
```

## 倉庫結構

```
.cargo/config.toml          發佈 LTO 與原生 CPU 最佳化
.github/workflows/ci.yml    CI：build、clippy、test、fmt
.opencode/                  OpenCode 計畫與內部狀態
examples/
  opencode.jsonc            MCP 客戶端配置範例
scripts/
  run_llama_fastcontext_rl.ps1/sh    llama.cpp 啟動腳本
  run_sglang_fastcontext_rl.ps1/sh   SGLang 啟動腳本
src/
  main.rs                   完整 MCP 伺服器（約 870 行）
  （測試在檔案底部 #[cfg(test)] mod 中）
install.ps1 / install.sh    安裝至 ~/.cargo/bin
uninstall.ps1 / uninstall.sh
README.md / README.zh-TW.md   使用者文件
AGENTS.md / AGENTS.zh-TW.md   Agent 文件（本文件）
Cargo.toml                   相依性：tokio、serde、anyhow
lessons.md                   RSI 經驗教訓
```

## 程式碼慣例

- **單一檔案：** 所有邏輯在 `src/main.rs` 中。保持集中；只有在超過 ~1200 行時才提取模組。
- **錯誤處理：** 使用 `anyhow::Result` / `bail!` / `Context`。正式環境程式碼中不使用 unwrap（測試中可以）。
- **MCP 協定：** JSON-RPC 2.0，透過 stdin/stdout 傳輸。每行是一個換行分隔的 JSON。
- **配置：** 透過 `Config::from_env()` 從環境變數讀取。工具引數可覆蓋每個請求的設定。

## 建置與測試指令

```bash
cargo check                    # 快速編譯檢查
cargo clippy --all-targets --all-features  # Lint（必須完全通過）
cargo test                     # 執行單元測試（目前 31 個）
cargo fmt --check              # 格式化檢查
cargo build --release          # 發佈建置（含 LTO）
```

## 關鍵架構決策

### 提供的工具：

1. **`fastcontext_explore`** — 主要工具。將 `fastcontext` CLI 作為子程序啟動並使用 piped I/O。
   引數：`query`（必填）、`work_dir`、`max_turns`、`citation`、`trajectory_file`、
   `timeout_secs`、`verbose`、`base_url`、`model`、`api_key`。

2. **`fastcontext_status`** — 唯讀診斷工具。回傳配置、二進位檔狀態、環境變數狀態。

### 安全性：

- `work_dir` 會針對 `FASTCONTEXT_ALLOWED_ROOT` 進行驗證（正規化路徑檢查）。
- `trajectory_file` 必須是相對路徑；絕對路徑和父目錄路徑會被拒絕。
- FastContext 子程序有可設定的逾時（預設 300 秒）。
- 伺服器刻意設計為唯讀——不執行 shell、不寫入檔案、不修改程式碼。

### 啟動診斷：

啟動時，伺服器檢查：
- `fastcontext` 二進位檔是否在 PATH 中
- `BASE_URL` 和 `MODEL` 環境變數是否已設定
- 對缺少的依賴項目印出警告

## 測試

所有測試在 `src/main.rs` 底部的 `#[cfg(test)] mod tests` 中。
目前有 31 個測試，涵蓋：
- 路徑驗證（相對、絕對、父目錄、根目錄）
- 錯誤截斷
- 工具列表結構
- JSON-RPC 回應格式
- ExploreArgs 反序列化（含 base_url/model/api_key）
- 請求處理（initialize、tools/list、ping、unknown、notification）
- Config::resolve_work_dir
- server_status_text 輸出
- check_fastcontext_binary

## 修改時應注意

1. 新增或修改工具輸入結構時，更新 `tools_list_result()`。
2. 新增工具時，更新 `handle_tools_call()` 的分派邏輯。
3. 新增環境變數時，更新 `main()` 中的啟動診斷。
4. 為新功能新增測試。
5. 針對結構性變更更新 `AGENTS.md`。
6. 執行完整驗證：`cargo fmt --check && cargo clippy --all-targets --all-features && cargo test`
