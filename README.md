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

Unzip the release archive and run `install.command` (right-click → Open the
first time). It removes the quarantine flag and copies the bundle to
`~/Library/Audio/Plug-Ins/CLAP`.

Manual install:

```bash
mkdir -p ~/Library/Audio/Plug-Ins/CLAP
cp -R Pivot.clap ~/Library/Audio/Plug-Ins/CLAP/
```

For a system-wide install, use `/Library/Audio/Plug-Ins/CLAP` with `sudo`
instead.

The release is ad-hoc signed, not notarized. If macOS quarantines the bundle,
remove the quarantine flag:

```bash
xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/CLAP/Pivot.clap
```

FL Studio 2024.1 or later is required for CLAP support.

## Build from source

Requires Rust stable and the Xcode command line tools.

```bash
cargo build
cargo test
cargo xtask bundle pivot --release
cargo xtask bundle-universal pivot --release
```

Bundles are written to `target/bundled/`. The release profile uses
`strip = "none"` because the current macOS linker produces a malformed symbol
string pool with the default debuginfo stripping.

## Release

```bash
./scripts/package.sh
```

This creates `dist/Pivot-1.0.0-macOS-universal.zip`. The bundle is ad-hoc
signed. Public distribution without Gatekeeper warnings requires a Developer ID
certificate and notarization.

## Validation

- `cargo test`: 8 tests pass.
- CLAP: passes 34 of 44 clap-validator tests with 0 failures and 10 skipped.
  The vendored nih-plug wrapper fixes CLAP state loading and malformed-state
  handling; see `SPEC.md` section 12.4.

## Source

Pivot is open source under the MIT License.

- GitHub: <https://github.com/Iranjin/Pivot>
- Issues and pull requests are welcome.

## Licenses

Pivot is released under the MIT License. The plugin embeds its own license,
the third-party notices, the per-component copyright list and the full license
texts in the editor. Click **Licenses** at the bottom of the window to read
them. No separate license file is installed with the CLAP bundle.

## Documentation

- [SPEC.md](SPEC.md) is the source of truth for behavior.
- [AGENTS.md](AGENTS.md) contains project instructions.
- [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) lists dependency notices.
