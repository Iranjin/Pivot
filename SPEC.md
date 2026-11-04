# Pivot Specification

- Version: 1.0
- Last updated: 2026-11-04
- Target: macOS audio pitch-shift CLAP plugin
- Implementation language: Rust (nih-plug + egui)

---

## 1. Overview

### 1.1 Concept

**Pivot** is an audio effect that changes the key (pitch) of incoming audio in
real time, in a musically meaningful and freely controllable way.

Beyond simple semitone transposition, it handles all of the following in a single
plugin:

- Continuous pitch shifting (semitones and cents)
- Manual key-based transposition from a source key to a target key
- Pitch snapping to the notes of a scale
- Morphing between keys (glide)
- Pitch modulation via LFO and MIDI

As the name suggests, morphing the key is the core purpose.

### 1.2 One-line description

> A pitch-shift plugin that lets you move the key of any source in real time,
> musically, and freely.

### 1.3 Intended users

- Engineers and track makers who need vocals or instruments to match a song's key
- Producers matching the key of samples for remixes and mashups
- Sound designers who want to automate key as a performance parameter

---

## 2. Scope

### 2.1 In scope for v1.0

- Stereo-in / stereo-out audio effect
- Pitch shift (±24 semitones, 0.01 st resolution)
- Single Signalsmith Stretch pitch-shift engine
- Manual key-based transposition
- Scale snap (most effective on monophonic and vocal sources)
- Formant preservation / shift
- Glide (morphing between keys)
- LFO modulation
- Dry/wet, input/output gain, safety limiter
- Host state persistence
- CLAP bundle, macOS 12+ (Universal Binary)

### 2.2 Out of scope for v1.0 (future candidates)

- AAX / AU output (AU is a v1.1 candidate)
- MIDI pitch bend / CC modulation
- Harmony generation (additional voices such as +3rd / +5th)
- Full-mix processing that embeds stem separation
- Windows / Linux support
- AI-based timbre conversion

---

## 3. Terminology

| Term | Definition |
|---|---|
| Key | The tonality of a piece: a root note (C–B) plus a mode (Major / Minor, etc.) |
| Semitone | One step of 12-tone equal temperament; 100 cents |
| Cent | 1/100 of a semitone. The human discrimination threshold is about 5–10 cents |
| Formant | A resonance frequency of the vocal tract. Separating it is necessary to preserve timbre when pitch changes |
| Scale snap | Processing that pulls detected pitches toward the notes of a specified key |
| Glide | A smooth transition of the pitch-shift amount over time |
| Latency | Delay from input to output. It must be reported to the host |

---

## 4. Functional Requirements

### 4.1 Pitch-shift engine

#### FR-1 Manual pitch shift

- `Pitch`: −24 to +24 semitones in 0.01 st steps (automation supported)
- Host automation is normalized 0.0–1.0, mapped linearly to −24..+24 st
- Apply 20 ms smoothing to changes to prevent clicks
- `Glide` sets the transition time for changes from 0 to 2000 ms

#### FR-2 Key-based transposition

- Set the source key manually (root + mode)
- Set the target key (root + mode)
- Applied shift = Target Key − Source Key (in semitones)
  - Example: Source = A minor, Target = C minor → +3 semitones
- Choose the shortest offset within ±6 semitones
- The key shift is added to the manual pitch amount

#### FR-3 Scale snap (monophonic / vocal sources)

- Pull each input note toward the scale notes of the Target Key
- `Snap Amount`: 0–100% (0 = no snap, 100 = snap fully to scale notes)
- `Retune Speed`: 0–200 ms (0 = immediate; larger values retune more gently)
- Scale presets: Major, Natural Minor, Harmonic Minor, Melodic Minor, Dorian,
  Phrygian, Lydian, Mixolydian, Locrian, Major Pentatonic, Minor Pentatonic,
  Blues, Chromatic
- In sections detected as polyphonic, fade the snap correction toward 0 based on
  the monophonicity confidence

#### FR-4 Single engine and quality presets

There is no user-facing mode switch. The engine uses Signalsmith Stretch, which
handles monophonic, polyphonic, and mixed material without a mode switch.

| Quality preset | Configuration | Primary use |
|---|---|---|
| Low | `preset_cheaper` | Lowest CPU |
| Balanced | `preset_default` | Middle ground |
| High | 0.14 s block, 0.02 s interval | Default; highest quality |

- The plugin reports a fixed latency equal to the largest preset latency; shorter
  presets are padded so the host never needs a latency update
- Quality presets trade CPU and artifacts, not latency

#### FR-5 Formant control

- `Formant`: −12 to +12 semitones (shifts formants only)
- `Formant Preserve`: 0–100% (100 = keep formant positions after pitch shifting)
- Map the controls to Signalsmith Stretch's formant factor with pitch compensation
- Use the same processing chain for monophonic, polyphonic, and mixed material
- Formant shift operates independently of pitch shift

#### FR-6 Modulation

- LFO
  - Rate: 0.01–20 Hz
  - Depth: 0–12 semitones
  - Shape: Sine / Triangle / Square / Saw / Random
  - Phase: 0–360°
  - Target: Pitch / Mix
- Automation
  - All primary parameters support host automation
  - Pitch / Target Key are applied with sample accuracy

#### FR-7 Mix and gain

- `Mix`: 0–100% (dry/wet, equal-power curve)
- `Input Gain` / `Output Gain`: −24 to +24 dB
- `Safety Limiter`: on/off (output −0.3 dBFS, internal to the plugin only)
- When pitch shift is 0 and Mix is 100%, guarantee bit-identical output to the dry
  path

### 4.2 Pitch analysis

- Downmix the stereo input to mono
- STFT: 4096 points, hop 2048, Hann window
- F0: harmonic product spectrum (40–800 Hz, five harmonics) with parabolic
  refinement
- Monophonicity: spectral flatness and harmonic energy, smoothed over frames
- Update rate: every 2048 samples (about 43 ms at 48 kHz)
- Used for scale snap only; there is no automatic key detection

### 4.3 State

- Persist plugin state through the host (CLAP state)
- The custom UI exposes only the pitch knob; all other parameters are exposed
  through the host's generic parameter list
- No plugin-internal preset browser or A/B snapshots

### 4.4 UI

#### Layout (460 × 520 px, fixed)

- Main view: title and one large pitch knob (up to 380 px).
- No bypass, mix, latency, or advanced controls in the custom UI.
- All other parameters are available through the host's generic parameter list.

Knob interaction:

- Drag vertically for coarse pitch changes.
- Shift-drag for cent-level changes.
- Scroll for fine steps; Shift-scroll for finer steps.
- Double-click to type a value directly; Enter commits and Escape cancels.
- Entering `0` resets the pitch.

#### UI requirements

- Custom rendering with egui. Target 60 fps
- The pitch knob is the primary control and drives the single `Pitch` parameter
- When the editor has focus, pass transport and other non-navigation keys to the
  host unless a text field is active
- Do not use the default egui look; apply a custom theme (colors, corner radii,
  spacing, fonts)
- Draw the pitch knob from scratch with a `Widget` implementation and `Painter`
- Use `egui_glow` callbacks for shader-based rendering where useful
- Support HiDPI and remain intact at 150% / 200% scaling
- The knob shows the value inside and supports drag, Shift-drag, scroll, and
  double-click text entry
- A small Licenses link opens an in-editor viewer containing the MIT license
  and third-party notices
- License texts are embedded in the binary; no separate license file is
  installed with the plugin bundle

---

## 5. Non-functional Requirements

### 5.1 Supported environment

| Item | Requirement |
|---|---|
| OS | macOS 12 Monterey or later |
| CPU | Apple Silicon (arm64) / Intel (x86_64) Universal Binary |
| Plugin formats | CLAP 1.x |
| DAW validation | FL Studio 2024.1+, Bitwig, Reaper, Studio One (CLAP) |
| Sample rates | 44.1 / 48 / 88.2 / 96 / 176.4 / 192 kHz |
| Channels | Stereo in → stereo out (mono in/out also accepted) |
| Buffer size | 32–2048 samples |

### 5.2 Performance

- Latency (48 kHz, the value reported to the host): 6720 samples (140 ms) for
  every quality preset
  - The engine uses the largest preset latency and pads shorter presets to it
  - Quality presets trade CPU and artifacts, not latency

- CPU: Apple M1, 48 kHz / 128 samples, Balanced: ≤ 15% for one instance
  (relative to one core)
- ≤ 30% even for mixed material at High
- Memory: ≤ 100 MB per instance
- No memory allocation on the audio thread (preallocate all buffers at startup)
- Implement denormal protection

### 5.3 Stability

- No crashes or memory leaks during 48 hours of continuous operation
- Preserve state across sample-rate and buffer-size changes
- No anomalies after 1000 plugin load/unload cycles
- No divergence with silence, DC, or full-scale input
- If NaN / Inf appears at the input, mute the affected block and recover

### 5.4 Licensing / distribution

- Plugin license: MIT
- License texts are embedded in the editor; release bundles contain no separate
  license file
- Dependencies must use permissive licenses compatible with MIT distribution
  (MIT, ISC, Apache-2.0, BSD, OFL, UFL, Unicode)
- Do not use GPL-licensed code (for example, Rubber Band)
- Code-sign and notarize releases
- Distribution format: `.pkg` or `.dmg` (bundling CLAP)

---

## 6. DSP Design

### 6.1 Signal chain

```
Input (stereo)
  │
  ├─ Input Gain ──► Key / F0 Analyzer ──► Key Detector
  │
  ▼
Signalsmith Stretch (pitch + formant)
  │
  ├─ LFO Pitch Modulation
  ├─ Dry/Wet Mix
  ├─ Output Gain
  └─ Safety Limiter ──► Output (stereo)
```

### 6.2 Pitch engine (Signalsmith Stretch)

- Signalsmith Stretch (MIT) handles pitch shifting and formant correction
- Audio is processed in fixed 64-frame chunks; pitch and formant settings are
  updated per chunk
- Quality presets:
  - Low: `preset_cheaper`
  - Balanced: `preset_default`
  - High: 0.14 s block, 0.02 s interval
- The engine reports the largest preset latency and pads the shorter presets to it
- The dry path is delayed by the same amount so the mix stays aligned

### 6.3 Formant mapping

- `preserve` is 0–1, `F_shift = 2^(formant/12)`, and `P = 2^(pitch/12)`
- Desired output formant ratio = `F_shift * P^(1-preserve)`
- Set the Signalsmith formant multiplier to `F = P^preserve / F_shift` with
  `compensate_pitch = true`
- `preserve = 1` keeps formants in place; `preserve = 0` lets them follow the pitch
- When `preserve = 0` and `formant = 0`, disable formant processing entirely

### 6.4 Mono / polyphonic handling

- Signalsmith Stretch processes monophonic, polyphonic, and mixed material without
  a user-facing mode switch
- The key analyzer still estimates F0 and monophonicity for scale snap
- No mode switch is exposed in the UI

### 6.5 Latency management

- At 48 kHz the largest preset latency is 6720 samples (140 ms)
- Report that value once during initialization; quality changes do not change it
- Pad the wet path of shorter presets and delay the dry path by the same amount
- Do not offer a zero-latency mode

### 6.6 Pitch analysis details

- Input: downmixed signal (L+R)/2
- STFT: 4096 points, hop 2048, Hann window
- F0: harmonic product spectrum (40–800 Hz, five harmonics) with parabolic
  refinement
- Monophonicity: spectral flatness and harmonic energy, smoothed over frames
- Update rate: every 2048 samples (about 43 ms at 48 kHz)
- Used for scale snap only

---

## 7. Parameter Specification

### 7.1 Global

| ID | Display | Range | Default | Unit | Smoothing | Notes |
|---|---|---|---|---|---|---|
| `bypass` | Bypass | ON/OFF | OFF | — | — | Host integration |
| `quality` | Quality | Low / Balanced / High | High | — | — | Changes CPU and artifacts |
| `mix` | Mix | 0–100 | 100 | % | 20 ms | Equal power |
| `input_gain` | Input | −24 to +24 | 0 | dB | 20 ms | — |
| `output_gain` | Output | −24 to +24 | 0 | dB | 20 ms | — |
| `limiter` | Limiter | ON/OFF | ON | — | — | −0.3 dBFS |

### 7.2 Pitch

| ID | Display | Range | Default | Unit | Smoothing | Notes |
|---|---|---|---|---|---|---|
| `pitch` | Pitch | −24 to +24 | 0 | st | 20 ms | 0.01 st steps, sample-accurate |
| `glide` | Glide | 0–2000 | 20 | ms | — | 0 = immediate |
| `formant_shift` | Formant | −12 to +12 | 0 | st | 20 ms | — |
| `formant_preserve` | Preserve | 0–100 | 100 | % | 20 ms | Most effective on monophonic sources |

### 7.3 Key

| ID | Display | Range | Default | Unit | Notes |
|---|---|---|---|---|---|
| `source_root` | Source Root | C–B | C | — | — |
| `source_mode` | Source Mode | Major / Minor | Minor | — | — |
| `target_root` | Target Root | C–B | C | — | — |
| `target_mode` | Target Mode | Major / Minor | Minor | — | — |
| `snap_amount` | Snap | 0–100 | 0 | % | Monophonic sources |
| `retune_speed` | Retune | 0–200 | 30 | ms | Monophonic sources |
| `scale` | Scale | 13 types | Major | — | Monophonic sources |

### 7.4 Modulation

| ID | Display | Range | Default | Unit | Notes |
|---|---|---|---|---|---|
| `lfo_shape` | LFO Shape | Sine / Tri / Square / Saw / Random | Sine | — | — |
| `lfo_rate` | LFO Rate | 0.01–20 | 0.5 | Hz | — |
| `lfo_depth` | LFO Depth | 0–12 | 0 | st | — |
| `lfo_phase` | LFO Phase | 0–360 | 0 | ° | — |
| `mod_target` | LFO Target | Pitch / Mix | Pitch | — | — |

---

## 8. Plugin Specification

### 8.1 Identifiers

| Item | Value |
|---|---|
| Plugin name | Pivot |
| Vendor | Iranjin |
| CLAP ID | `com.iranjin.pivot` |
| Version | 1.0.0 |

### 8.2 Buses / events

- Main input: stereo (mono also accepted)
- Main output: stereo
- Sidechain: none
- MIDI input: yes (pitch bend, CC, aftertouch)
- MIDI output: none
- Tempo information: used for LFO sync (CLAP transport)

### 8.3 State

- Expose all parameters to the host and allow automation
- State serialization is handled by nih-plug
- The editor size is persisted with the plugin state

### 8.4 Host integration

- Report latency accurately
- Tail: 200 ms (engine residue)
- Bypass passes audio through completely (and stops CPU work)
- Reinitialize the engine on sample-rate changes
- Offline rendering (higher quality) is planned for v1.1

---

## 9. Technology Selection

### 9.1 Language / framework

Use Rust + nih-plug.

| Item | Value |
|---|---|
| Language | Rust (stable, 2024 edition) |
| Plugin framework | nih-plug (CLAP) |
| UI | egui (`nih_plug_egui` / `egui_glow` backend) |
| Licensing | nih-plug: ISC / egui: MIT or Apache-2.0 |

Reasons:

1. Produce a CLAP plugin with a small, auditable dependency graph
2. Enforce no allocation on the audio thread by design
3. egui offers enough custom drawing freedom to meet the UI requirements
4. Compose the stack from MIT / ISC / Apache-2.0 only and avoid GPL

### 9.2 UI: egui customization

egui is an immediate-mode GUI like Dear ImGui, and its visual customization range
is equal to or greater than ImGui's. It is not constrained to the default look.

| Technique | What it enables |
|---|---|
| Style / Visuals | Change colors, corner radii, spacing, shadows, fonts, line height globally |
| Custom widget | Implement the `Widget` trait to define arbitrary drawing and interaction |
| Painter | Draw lines, circles, Béziers, gradients, and text directly |
| Images / SVG | Place logos, icons, and backgrounds |
| Animation | Redraw every frame; control value interpolation yourself |
| `egui_glow` callback | Fully custom shader rendering (glow, spectrum, etc.) |
| Fonts | Embed TTF / OTF files |

- Relative to ImGui, the only disadvantages are the number of ready-made themes
  and the depth of C++ ecosystem integration
- Implement the keyboard, circle of fifths, meters, and knobs entirely with Painter
  and custom widgets
- Do not use the default egui look; build a custom theme from the start

### 9.3 Dependencies (proposed)

| Crate | Purpose | License |
|---|---|---|
| `nih_plug` (git, pinned revision) | Plugin framework | ISC |
| `nih_plug_xtask` (git, pinned revision) | Plugin bundler | ISC |
| `nih_plug_egui` | egui editor | ISC |
| `egui` (via `nih_plug_egui`; `eframe` for standalone tests) | UI | MIT / Apache-2.0 |
| `realfft` | Real FFT | MIT |
| `num-complex` | Complex number types | MIT / Apache-2.0 |
| `signalsmith-stretch` | Pitch/time engine | MIT |

- The pitch shifter uses Signalsmith Stretch (MIT)
- Build requires a C++ compiler and libclang (for bindgen)
- Do not use the `rubberband` crate because it is GPL

---

## 10. Architecture

### 10.1 Threading model

```
┌──────────────┐    atomic params    ┌──────────────────┐
│  UI thread   │ ──────────────────► │  Audio thread    │
│  (egui)      │ ◄────────────────── │  (process +      │
└──────────────┘   meters / latency  │   analysis)      │
        │                             └──────────────────┘
        │ preset save/load
        ▼
┌──────────────┐
│  Disk / Host │
│  state       │
└──────────────┘
```

- Share parameters through `AtomicFloat` / `AtomicEnum`
- Run pitch analysis in the audio thread with preallocated buffers
- Poll state from the UI at 30–60 fps (no locks)
- On the audio thread, prohibit `Vec` reallocation, `Mutex`, and file I/O

### 10.2 Directory layout

```
Pivot/
├── .cargo/config.toml      # cargo xtask alias
├── Cargo.toml
├── SPEC.md
├── README.md
├── AGENTS.md
├── CLAUDE.md
├── bundler.toml            # bundle display name
├── xtask/                  # bundling
│   ├── Cargo.toml
│   └── src/main.rs
└── src/
    ├── lib.rs              # plugin, editor, presets
    ├── params.rs           # parameters and enums
    ├── ui.rs               # pitch knob widget
    └── dsp/
        ├── mod.rs
        ├── engine.rs       # Signalsmith Stretch wrapper
        └── key_detector.rs
```

---

## 11. Build / Distribution

### 11.1 Development

```bash
# Fetch dependencies (nih-plug is pinned to a git revision)
cargo build

# Build bundles
cargo xtask bundle pivot --release
# → target/bundled/Pivot.clap

# Universal release bundles (arm64 + x86_64)
cargo xtask bundle-universal pivot --release
```

### 11.2 Release

1. Pass `cargo test` / `cargo clippy`
2. `./scripts/package.sh`
3. Validate with clap-validator
4. For public distribution, code-sign with a Developer ID Application
   certificate and notarize with `notarytool`
5. Create a `.dmg` and attach the ticket with `stapler`

### 11.3 CI

- GitHub Actions (macOS runner)
- `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test`
- Automate bundling and notarization on tag pushes (keep certificates in Secrets)

---

## 12. Testing / Acceptance Criteria

### 12.1 Unit tests

- Pitch detection: within ±5 cents on a 40–1000 Hz sine sweep
- Scale snap: a detuned note is corrected to the nearest scale degree
- FFT / iSTFT: covered by Signalsmith Stretch
- Engine latency: impulse peak at the reported latency (±1 sample)
- Pitch shift: 440 Hz → 880 Hz within 10 Hz after the latency
- Formant control: formant shift moves spectral energy in the expected direction
- Monophonic detection: a sine is classified as monophonic and reports its F0
- Latency: difference between reported and measured values within 1 sample, using
  an impulse response; all quality presets report 6720 samples at 48 kHz
- State: all parameters match after host save → load
- Parameter range: no divergence when feeding boundary values, NaN, and Inf into
  every parameter

### 12.2 Integration tests

- Bit-identical to dry at pitch 0 / Mix 100% (null test)
- Click noise below −60 dBFS when shifting ±24 semitones
- Output stays at or below −0.3 dBFS when the limiter is enabled
- No dropouts or clicks when changing quality presets
- Works at sample rates from 44.1 to 192 kHz
- Works with buffers from 32 to 2048 samples

### 12.3 Validation tools

- clap-validator
- Manual DAW checks (FL Studio / Bitwig / Reaper / Studio One)

### 12.4 Known validation exceptions

- The vendored nih-plug CLAP wrapper requests a parameter value rescan after
  loading host state, so parameter changes are reflected without a host-side
  rescan.
- The vendored wrapper rejects state streams larger than 16 MiB before
  allocating, preventing malformed state from causing an allocation failure.
- clap-validator passes 34 of 44 tests with 0 failures and 10 skipped tests.

### 12.5 Acceptance criteria (v1.0)

- [ ] Pitch shifting works on monophonic, polyphonic, and mixed material
- [ ] Manual key shift works
- [ ] Scale snap works on monophonic sources
- [ ] Latency is reported correctly
- [ ] CPU stays within target
- [ ] No crashes in major DAWs
- [ ] Presets can be saved and loaded
- [ ] A code-signed / notarized installer is available

---

## 13. Milestones

| Phase | Content | Estimate |
|---|---|---|
| M0 | Project skeleton, CLAP output, bypass + gain | 1 week |
| M1 | Signalsmith Stretch integration, parameters, latency reporting | 2–3 weeks |
| M2 | Formant mapping, pitch analysis, scale snap | 2–3 weeks |
| M3 | Key UI, scale snap, LFO, limiter | 2 weeks |
| M4 | LFO, limiter, UI polish | 1–2 weeks |
| M5 | Validation, optimization, Universal Binary, signing / notarization, release | 1–2 weeks |

---

## 14. Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| F0 misdetection in monophonic passages | Medium | Confidence gating, smoothing, manual source key |
| Formant artifacts at large shifts | Medium | Preserve slider, Signalsmith formant compensation |
| CPU load on mixed material | Medium | Quality presets, High only when needed |
| Latency vs. quality trade-off | Medium | Fixed 140 ms reported latency at 48 kHz, host compensation, no zero-latency mode |
| Build complexity from Signalsmith FFI | Low | Pinned crate, prebuilt buffers, CI build check |
| Notarization / signing operational burden | Low | Automate in CI, manage certificates in Secrets |
| CLAP state-load rescan (upstream nih-plug) | Medium | Document and track an upstream fix |
| CLAP malformed-state allocation (upstream nih-plug) | Low | Document; valid host state is unaffected |
| Out-of-tune feel from scale snap | Medium | Separate snap amount from retune speed; make both preset-adjustable |

---

## 15. Open Questions

- [x] License (MIT)
- [ ] Vendor name and CLAP ID
- [ ] Price / distribution channel (direct / App Store / free)
- [ ] Include AU support in v1.1?
- [ ] Include harmony generation in v1.0?
- [ ] Visual tone of the UI (colors, fonts, texture)
- [ ] Custom scale editor (per-note on/off)
- [ ] MIDI pitch bend / CC modulation
- [ ] How far to support Intel Macs (is a Universal Binary mandatory)?

---

## 16. References

- nih-plug: https://github.com/robbert-vdh/nih-plug
- CLAP: https://github.com/free-audio/clap
- Signalsmith Stretch: https://github.com/Signalsmith-Audio/signalsmith-stretch
- Signalsmith Linear: https://github.com/Signalsmith-Audio/linear
- Harmonic product spectrum: Schroeder (1968), *Period histogram and product spectrum: New methods for fundamental-frequency measurement*
