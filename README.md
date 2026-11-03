# KeyMorph

Real-time pitch and key shifting audio effect for macOS.

- Formats: VST3, CLAP
- Stack: Rust, nih-plug, egui
- Specification: [SPEC.md](SPEC.md)

## Build

```bash
cargo build
cargo xtask bundle keymorph --release
cargo xtask bundle-universal keymorph --release
```

Bundles are written to `target/bundled/`.
