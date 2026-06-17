# Prompt Reference

參考來源：[Microsoft FastContext prompts](https://github.com/microsoft/fastcontext/tree/main/prompts)

## 目錄結構

| 檔案 | 用途 |
|------|------|
| `gpt-multi-fc.yaml` | GPT-4o 多輪 SWE 修復 agent（/testbed） |
| `gpt-pro-fc.yaml` | GPT-4o 多輪 SWE 修復 agent（/app） |
| `gpt-qa-fc.yaml` | GPT-4o 問答 agent，回答程式碼問題 |
| `glm-kimi-multi-fc.yaml` | GLM/Kimi 版 multi-fc |
| `glm-kimi-pro-fc.yaml` | GLM/Kimi 版 pro-fc |
| `glm-kimi-qa-fc.yaml` | GLM/Kimi 版 qa-fc |

## 重要發現

### 1. 這些是「主 agent」prompt，非 FastContext 子 agent prompt

YAML 檔定義的是 **Mini-SWE-Agent 主 agent** 的 system prompt，其中包含一段 `## fastcontext` 的指引，說明**何時以及如何呼叫 fastcontext 子 agent**。

FastContext 子 agent 本身的 system prompt 是**訓練到模型權重中**或在 `fastcontext` CLI 原始碼中硬編碼，不在這些 YAML 檔裡。

### 2. `--citation` 格式

主 agent 以 `fastcontext -q "<query>" --citation` 呼叫 fastcontext。`--citation` 讓 fastcontext 輸出 `<final_answer>` XML 區塊：

```
<final_answer>
Summary: ...

Files:
- src/auth.rs:15-42  — Authentication middleware
...
</final_answer>
```

### 3. 使用時機決策

主 agent 文件中明確列出何時 **跳過** fastcontext：

- PR 描述已指名檔案/符號
- 前一次 turn 已取得需要的路徑
- 只需要讀一個已知檔案
- 在 2-3 個已知檔案中搜尋

何時 **使用**：

- 需要發現功能或符號在 repo 哪裡
- 需要跨多檔案的結構化列表
- 直接 grep 搜尋無結果

### 4. 閱讀規範

```
B - A <= 80          — 單次讀取最多 80 行
sed -n 'A,Bp'        — 精準行號讀取
batch over expand    — 批量平行讀取代價一小步擴張
| head -n 80         — grep 輸出上限
no re-reads          — 已讀過的區段不重讀
```

## 對本專案的影響

本專案 `src/main.rs` 中的 `system_prompt()` 是 **FastContext 子 agent 的 prompt**，與官方 repo 中主 agent prompt 不同層次。不過以下要素值得採用：

| 要素 | 已採用 |
|------|--------|
| `<final_answer>` 引用格式 | ✅ 已加入 system_prompt |
| Broad → narrow 策略 | ✅ 已加入 |
| Batch tool calls | ✅ 已加入 |
| 精準行號範圍 | ✅ 內建於 read 工具 |
| 何時跳過探索 | 可選改進 |

## system_prompt 當前版本

定義在 `src/main.rs`，推給 FastContext LLM 作為開場 system message。
