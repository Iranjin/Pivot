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
- In-editor license viewer; no license files are installed with the plugin
- CLAP, universal binary (arm64 + x86_64)

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

## Install

```bash
sudo mkdir -p /Library/Audio/Plug-Ins/CLAP
sudo cp -R target/bundled/Pivot.clap /Library/Audio/Plug-Ins/CLAP/
```

FL Studio 2024.1 or later is required for CLAP support.

## Build

```bash
cargo build
cargo test
cargo xtask bundle pivot --release
cargo xtask bundle-universal pivot --release
```

Bundles are written to `target/bundled/`. The release profile uses
`strip = "none"` because the current macOS linker produces a malformed symbol
string pool with the default debuginfo stripping.

## Validation

- `cargo test`: 8 tests pass.
- CLAP: passes 30 of 44 clap-validator tests. The remaining failures are upstream
  nih-plug issues with state loading; see `SPEC.md` section 12.4.

## Licenses

Pivot is released under the MIT License. The plugin embeds its own license and
the third-party notices in the editor. Click **Licenses** at the bottom of the
window to read them. No separate license file is installed with the CLAP bundle.

## Documentation

- [SPEC.md](SPEC.md) is the source of truth for behavior.
- [AGENTS.md](AGENTS.md) contains project instructions.
- [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) lists dependency notices.
