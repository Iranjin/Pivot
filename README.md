# Pivot

Real-time pitch and key shifting audio effect for macOS.

## Features

- Large central pitch knob with 0.01-semitone resolution
- Signalsmith Stretch pitch shift, ±24 semitones
- Quality presets: cheaper, default, and high-quality configurations
- Formant shift and preservation
- Manual key-based transposition
- Scale snap for monophonic sources
- LFO modulation and a safety limiter
- Single-knob custom UI; all other parameters are exposed to the host
- VST3 and CLAP, universal binary (arm64 + x86_64)

The plugin reports a fixed latency of 6720 samples (140 ms at 48 kHz).

## Pitch knob

- Drag vertically for coarse changes.
- Shift-drag for cent-level changes.
- Scroll for fine steps; Shift-scroll for finer steps.
- Double-click to type a value; Enter commits and Escape cancels.
- Type `0` to reset to 0.

The knob drives the single `Pitch` parameter (−24 to +24 semitones, 0.01 st
steps). The host automates it as a normalized 0.0–1.0 value: 0.0 is −24 st,
0.5 is 0 st, and 1.0 is +24 st. All other parameters are available through the
host's generic parameter list and automation.

## Build

```bash
cargo build
cargo test
cargo xtask bundle pivot --release
cargo xtask bundle-universal pivot --release
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
- [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) lists dependency notices.
