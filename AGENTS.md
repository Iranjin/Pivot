# Pivot — Agent Instructions

These instructions apply to the entire repository.

## Language

- Write all project artifacts in English: source code, comments, commit messages,
  documentation, specifications, tests, UI strings, and issue/PR descriptions.
- Reply to the user in the language the user uses. Do not switch project artifacts
  to that language.

## Code comments

- Do not comment on code that is self-evident.
- Avoid redundant comments that restate what the code already says.
- Comment only on non-obvious rationale, constraints, invariants, or trade-offs.
- Comments must be in English.

## Project

Pivot is a real-time audio pitch/key-shift plugin.

- Language: Rust (stable, 2024 edition)
- Plugin framework: nih-plug (CLAP)
- UI: egui via `nih_plug_egui` / `egui_glow`
- Target: macOS 12+, Universal Binary
- `SPEC.md` is the source of truth for behavior. Update it when behavior changes.

## Build and test

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo xtask bundle pivot --release
```

## Engineering rules

- Do not allocate, lock, or perform file I/O on the audio thread.
- Report plugin latency accurately to the host.
- Do not add GPL-licensed dependencies. Allowed licenses: MIT, ISC, Apache-2.0.
- Keep DSP code testable without a running plugin host.

## Git

- Commit at logical, self-contained breakpoints. Do not commit after every small
  edit.
- Never push to a remote without explicit permission from the user.
- Write commit messages in English, in the imperative mood.
- Do not add co-author trailers.
- Never commit build artifacts, secrets, or signing credentials.

## Project policy updates

- When the user states a new project-wide rule or changes project direction, update
  this file in the same turn.
