# 安裝與開發注意事項

## PowerShell 腳本陷阱

### `$Host` 是 PowerShell 內建變數，不可作為參數名

**踩坑日期：** 2026-06-17  
**症狀：** 執行 `run_llama_fastcontext_rl.ps1` 報錯：
```
Cannot overwrite variable Host because it is read-only or constant.
```
**原因：** `$Host` 是 PowerShell 的自動變數（代表目前 host 環境），腳本將其作為參數名導致衝突。  
**解法：** 改名為 `$BindHost`。  
**教訓：** PowerShell 腳本參數命名時，避開 `$Host`、`$True`、`$False`、`$Null` 等自動變數。

---

### `llama-server` 不在 PATH 時腳本找不到

**踩坑日期：** 2026-06-17  
**症狀：** 腳本報 `llama-server not found`，但 binary 明明已下載到 `~/.config/llama-cpp/`。  
**原因：** 腳本只用 `Get-Command "llama-server"` 搜尋 PATH，而該目錄不在 PATH 上。  
**解法：** 先搜 PATH，找不到時 fallback 到 `~/.config/llama-cpp/llama-server.exe`。  
**教訓：** 非標準安裝路徑的工具，腳本應提供 fallback 機制或接受路徑參數。

---

## Rust 依賴陷阱

### ureq 3.x 與 2.x API 不相容

**踩坑日期：** 2026-06-17  
**症狀：**
- ureq 3.x：沒有 `.set()` 和 `.send_json()` 方法
- 切回 ureq 2.x 後，仍需啟用 `json` feature 才有 `.send_json()`

**正確寫法（Cargo.toml）：**
```toml
ureq = { version = "2", default-features = false, features = ["json"] }
```
**原因：** ureq 在 v3 進行了大幅 API 改寫（改用 `header()` 取代 `set()`，改用 `send()` 取代 `send_json()`）。  
**教訓：** 加入新版 crate 前，先確認 API 文件。若舊版 API 穩定且夠用，鎖定主版本。

---

### ureq 預設啟用 native-tls，在 Windows 上需注意

**踩坑日期：** 2026-06-17  
**建議：** 若只需 HTTP（本地模型伺服器），用 `default-features = false` 可跳過 TLS 依賴，加速編譯。  
**若需要 HTTPS：** 用 `features = ["json", "rustls"]` 避開 Windows 上 OpenSSL 的依賴問題。

---

## 測試陷阱

### 工具函數測試用字串出現在原始碼中導致自我匹配

**踩坑日期：** 2026-06-17  
**症狀：** `test_tool_grep_no_match` 測試失敗：
```
expected 'no matches', got: "src\\main.rs:887:  ...XYZZYX_NONEXISTENT_42..."
```
**原因：** 測試字串 `"XYZZYX_NONEXISTENT_42"` 被寫在 `main.rs` 中作為測試參數，導致 `tool_grep` 搜尋時在原始碼自身找到該字串。  
**解法：** 改用 `Some("*.nonexistent_ext_xyz")` 過濾副檔名，確保沒有檔案能匹配。  
**教訓：** 對專案自身進行搜尋測試時，測試內容不可出現在原始碼中。應使用：
- 不存在副檔名過濾（`*.nonexistent`）
- 或先將測試檔案移到隔離目錄
- 或先複製最小測試 fixture

---

## 模型部署陷阱

### 首次下載 GGUF 需等待數分鐘

**踩坑日期：** 2026-06-17  
**數據：** 從 HuggingFace 下載 `FastContext-1.0-4B-RL-Q4_K_M-GGUF`（2.49 GB）耗時約 **400 秒**（6.5 分鐘）。  
**原因：** `--hf-repo` 會在首次啟動時自動下載模型到 HuggingFace cache。  
**建議：**
- 首次啟動使用 `--ctx-size 32768` 而非 `262144`，以減少記憶體需求
- 可在啟動 script 前預先下載：
  ```powershell
  hf download mitkox/FastContext-1.0-4B-RL-Q4_K_M-GGUF --local-dir ~/Models/FastContext
  ```
- 第二次啟動時因已快取，約 10-30 秒即可載入

---

## OpenCode 配置陷阱

### 移除 subprocess 後需清理舊環境變數

**踩坑日期：** 2026-06-17  
**原因：** 從外部 `fastcontext` CLI 遷移到內建 agent loop 後，`FASTCONTEXT_BIN` 不再使用，若留在 `opencode.jsonc` 中只會造成混淆。  
**教訓：** 重大架構變更後，同步更新 OpenCode 設定檔中的環境變數。

---

## Git 注意事項

### CRLF / LF 警告

**每次編輯 PowerShell 腳本後都會出現：**
```
warning: in the working copy of 'install.ps1', LF will be replaced by CRLF the next time Git touches it
```
**原因：** 專案使用 Linux-style (LF) 換行，但 Windows 會轉換。  
**建議：** 加入 `.gitattributes`：
```
*.ps1   text eol=crlf
*.sh    text eol=lf
```
或維持現狀（無實際影響，僅警告）。
