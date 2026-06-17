---
## Lesson #1 — 2026-06-17
**Trigger:** Completing fastcontext-mcp-rust project — running `cargo fmt --check` revealed formatting drift on existing code after edits.
**Rule:** Always run `cargo fmt` immediately after making multiple edits to the same Rust file, then commit the formatted result — not before.
**Source:** fastcontext-mcp-rust completion plan

---

## Lesson #2 — 2026-06-17
**Trigger:** After `git init`, discovered that `${PROJECT_ROOT}` directory (a CBM artifact) was unintentionally tracked.
**Rule:** Before initial commit in a new repo, inspect `git status` carefully for stray/variable-expanded directories that shouldn't be versioned.
**Source:** fastcontext-mcp-rust completion plan

---

## Lesson #3 — 2026-06-17
**Trigger:** `.opencode/status-footer/state.json` kept changing during the session but was tracked in git.
**Rule:** Add OpenCode runtime state directories (`.opencode/status-footer/`) to `.gitignore` before initial commit to avoid constantly dirty tree.
**Source:** fastcontext-mcp-rust completion plan

---

## Lesson #4 — 2026-06-17
**Trigger:** Adding `base_url`/`model` tool arguments required modifying `ExploreArgs` struct and the CLI argument builder — a coordinated change across two layers.
**Rule:** When adding optional CLI passthrough parameters, update: (1) the args struct, (2) the input schema in `tools_list_result()`, (3) the async runner that builds the CLI command — in that order.
**Source:** fastcontext-mcp-rust improvement plan
