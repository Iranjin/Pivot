# KeyMorph

Real-time pitch and key shifting audio effect for macOS.

## Features

- Large central pitch knob with continuous semitone and cent control
- Phase-vocoder pitch shift, ±24 semitones and ±100 cents
- Quality presets with 1024, 2048, or 4096 point FFT windows
- Formant shift and preservation
- Automatic key detection and key-based transposition
- Scale snap for monophonic sources
- LFO modulation and a safety limiter
- A/B snapshots and versioned JSON presets
- VST3 and CLAP, universal binary (arm64 + x86_64)

The plugin reports a fixed latency of 4096 samples.

## Pitch knob

- Drag vertically for coarse changes.
- Shift-drag for cent-level changes.
- Scroll for fine steps; Shift-scroll for finer steps.
- Double-click to reset to 0.

The knob drives the Transpose and Fine parameters together.

## Build

```bash
cargo build
cargo test
cargo xtask bundle keymorph --release
cargo xtask bundle-universal keymorph --release
```

Bundles are written to `target/bundled/`.

## Validation

- VST3: passes pluginval at strictness level 10, including editor and parameter
  tests.
- CLAP: passes 30 of 44 clap-validator tests. The remaining failures are upstream
  nih-plug issues with state loading; see `SPEC.md` section 12.4.

## Documentation

- [SPEC.md](SPEC.md) is the source of truth for behavior.
- [AGENTS.md](AGENTS.md) contains project instructions.
