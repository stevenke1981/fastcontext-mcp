## Plan: fastcontext hardening
**Goal:** Harden repository path handling, improve agent-loop completion behavior, and align zh-TW docs with the inline agent architecture.
**Complexity:** L3

### Sub-tasks
1. [x] Harden path validation -> file: src/main.rs -> output: read/glob/grep cannot escape work_dir through absolute paths, parent dirs, or symlinks
2. [x] Improve agent loop fallback -> file: src/main.rs -> output: repeated tool calls and max-turn exhaustion return useful evidence instead of only failing
3. [x] Add regression tests -> file: src/main.rs -> output: safety and fallback behavior covered
4. [x] Sync docs and ignore rules -> files: README.zh-TW.md, docs/design-inline-agent-loop.md, .gitignore, lessons.md -> output: current behavior documented and runtime artifacts ignored
5. [x] Verify and persist -> output: fmt, clippy, tests pass; git commit and push complete

### Risks
| Risk | Mitigation |
|------|------------|
| Glob validation may reject existing useful patterns | Keep valid relative glob patterns such as `src/**/*.rs` working and add tests |
| Max-turn fallback could overclaim certainty | Prefix fallback with an explicit note that it is partial evidence |
| Symlink tests differ by Windows permissions | Use directory symlink on Windows and skip only when OS refuses creation |

### Definition of Done
- [x] Unsafe absolute, parent-dir, and symlink escape paths are rejected
- [x] Repeated tool calls and max-turn exhaustion produce a usable partial answer
- [x] zh-TW README no longer references removed CLI arguments
- [x] Runtime DB/index artifacts are ignored
- [x] `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` pass
- [x] git commit created and pushed

### Assumptions
- The current `master` branch push is authorized by the user's explicit "commit and push" request.
