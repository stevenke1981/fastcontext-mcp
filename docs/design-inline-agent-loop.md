# Inline Agent Loop: Replace fastcontext CLI with Rust-native LLM Explorer

**Date:** 2026-06-17  
**Status:** Implemented (2026-06-17)  
**Complexity:** L4 (cross-module, multi-file)

## Goal

Replace the external `fastcontext` CLI subprocess dependency with a Rust-native
LLM agent loop inside the MCP server. The server talks directly to the
FastContext model via OpenAI Chat Completions API, providing Read/Glob/Grep
tools for repository exploration.

## Architecture

```
fastcontext_explore(query)
  └─→ run_explorer()
       └─→ POST /v1/chat/completions (system + user + tool defs)
            └─→ LLM returns tool_calls → execute Read/Glob/Grep → loop
            └─→ LLM returns text (stop) → return evidence
```

## New Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `ureq` | 3 | Blocking HTTP client |
| `glob` | 0.3 | Glob file matching |
| `regex` | 1 | Grep content search |

## Tools Provided to LLM

| Tool | Description | Safety |
|------|-------------|--------|
| `read` | Read file with line numbers (max 200 lines) | Path sanitized, anchored to work_dir |
| `glob` | Find files by glob pattern (max 50 results) | Scoped to work_dir |
| `grep` | Regex content search (max 30 results) | Scoped to work_dir |

## Config Changes

- Remove `fastcontext_bin` field
- Remove `check_fastcontext_binary()` function
- Add `base_url: String`, `model: String`, `api_key: String` from env vars
- Keep `work_dir`, `allowed_root`, `max_turns`, `timeout_secs`

## ExploreArgs Changes

- Remove: `citation`, `trajectory_file`, `verbose`
- Keep: `query`, `work_dir`, `max_turns`, `timeout_secs`, `base_url`, `model`, `api_key`

## File Structure

Single `src/main.rs` (~1100 lines):
- MCP server routing (~280 lines, unchanged)
- Config & ExploreArgs (~80 lines, adjusted)
- `run_explorer()` agent loop (~120 lines)
- Tool execution: `tool_read/tool_glob/tool_grep` (~100 lines)
- LLM HTTP client wrapper (~60 lines)
- Path sanitization (~30 lines)
- Tests (~500 lines, updated)

## Agent Loop Pseudocode

```
messages = [system_prompt, user_query]
for turn in 0..max_turns:
    resp = llm_chat(messages, tools)
    if resp.finish_reason == "stop":
        return resp.content
    for tc in resp.tool_calls:
        result = execute_tool(tc.name, tc.args)
        messages.push(ToolMessage(tc.id, result))
```

## Error Handling

| Scenario | Behavior |
|----------|----------|
| LLM unreachable | Return clear error: "Cannot reach model server at {url}" |
| Invalid tool_calls | Skip, log warning, continue |
| Tool execution error | Return error text to LLM, let it recover |
| Max turns reached | Return accumulated context + note |
| Timeout | Same existing timeout mechanism |
