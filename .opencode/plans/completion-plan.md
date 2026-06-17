# fastcontext-mcp-rust Completion Plan

> **For agentic workers:** Execute tasks sequentially. Each task produces verifiable output.

**Goal:** Bring fastcontext-mcp-rust to production-ready state: zero warnings, tested code, git-standardized.

**Architecture:** Single-binary Rust MCP stdio server wrapping FastContext CLI. All logic in `src/main.rs` (384 lines). Tasks add tests in-file or in `tests/` directory, fix lint issues, and initialize git infrastructure.

**Tech Stack:** Rust 2021, tokio, serde, anyhow

---

### Task 1: Fix Compiler Warnings

**Files:**
- Modify: `src/main.rs:2` (remove `Serialize`)
- Modify: `src/main.rs:7` (remove `Path`)

- [ ] **Step 1: Remove unused `Serialize` import**

  `src/main.rs:2`: change `use serde::{Deserialize, Serialize};` to `use serde::Deserialize;`

- [ ] **Step 2: Remove unused `Path` import**

  `src/main.rs:7`: change `use std::path::{Component, Path, PathBuf};` to `use std::path::{Component, PathBuf};`

- [ ] **Step 3: Verify zero warnings**

  Run: `cargo check 2>&1`
  Expected: no warnings

---

### Task 2: Fix Clippy Lints

**Files:**
- Modify: `src/main.rs:323-325` (early return idiom)
- Modify: `src/main.rs:377-380` (single-match idiom)

- [ ] **Step 1: Replace early return with `?` operator**

  `src/main.rs:323-325`:
  ```rust
  // Old:
      if req.id.is_none() {
          return None;
      }
  // New:
      req.id.as_ref()?;
  ```

- [ ] **Step 2: Replace single-match with `if let`**

  `src/main.rs:377-380`:
  ```rust
  // Old:
          match handle_request(&config, req) {
              Some(out) => write_json(&out)?,
              None => {}
          }
  // New:
          if let Some(out) = handle_request(&config, req) {
              write_json(&out)?;
          }
  ```

- [ ] **Step 3: Verify clippy zero warnings**

  Run: `cargo clippy --all-targets --all-features 2>&1`
  Expected: no warnings

---

### Task 3: Add Unit Tests

**Files:**
- Modify: `src/main.rs` (append test module at bottom)

- [ ] **Step 1: Add test module with core unit tests**

  Append after line 384. Tests cover:
  - `validate_relative_path` — valid relative path, absolute path rejection, parent-dir rejection
  - `truncate_for_error` — short text unchanged, long text truncated, newline handling
  - `tools_list_result` — has `tools` key, has `fastcontext_explore` tool
  - `error_response` — structure matches JSON-RPC error spec
  - `response` — structure matches JSON-RPC success spec

- [ ] **Step 2: Run tests to verify they pass**

  Run: `cargo test 2>&1`
  Expected: all tests pass

---

### Task 4: Git Standardization

**Files:**
- Create: `.gitignore`
- Create: `.gitattributes`

- [ ] **Step 1: Create `.gitignore`**

  Standard Rust gitignore:
  ```
  /target/
  *.swp
  *.swo
  fastcontext.exe
  .fastcontext/
  ```

- [ ] **Step 2: Create `.gitattributes`**

  ```
  *.sh text eol=lf
  *.ps1 text eol=crlf
  *.rs text diff=rust
  *.json text
  *.md text
  ```

- [ ] **Step 3: Initialize git repo and create initial commit**

  ```bash
  git init
  git add -A
  git commit -m "chore: initial project setup

  fastcontext-mcp-rust v0.1.0
  Local MCP stdio wrapper for Microsoft FastContext CLI and FastContext-1.0-4B-RL"
  ```

---

### Task 5: Final Verification

- [ ] **Step 1: `cargo fmt --check`**
- [ ] **Step 2: `cargo clippy --all-targets --all-features`**
- [ ] **Step 3: `cargo test`**
- [ ] **Step 4: `git log --oneline -3` to verify commit history**
- [ ] **Step 5: `git status` to confirm clean working tree**

---

### Task 6: Report & Lessons

- [ ] **Step 1: Write `lessons.md` with RSI entries**
- [ ] **Step 2: Deliver Build Result report**

---

## Definition of Done

- [ ] `cargo check` — zero warnings
- [ ] `cargo clippy --all-targets --all-features` — zero warnings
- [ ] `cargo test` — all tests pass
- [ ] `cargo fmt --check` — clean
- [ ] Git initialized with `.gitignore`, `.gitattributes`
- [ ] Initial commit created
- [ ] `lessons.md` created
