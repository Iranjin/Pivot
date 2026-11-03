# KeyMorph Specification

- Version: 0.1 (draft)
- Last updated: 2026-11-03
- Target: macOS audio pitch-shift VST3 / CLAP plugin
- Implementation language: Rust (nih-plug + egui)

---

## 1. Overview

### 1.1 Concept

**KeyMorph** is an audio effect that changes the key (pitch) of incoming audio in
real time, in a musically meaningful and freely controllable way.

Beyond simple semitone transposition, it handles all of the following in a single
plugin:

- Continuous pitch shifting (semitones and cents)
- Key detection and transposition from a source key to a target key
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
- Pitch shift (±24 semitones, ±100 cents)
- Single Signalsmith Stretch pitch-shift engine
- Key detection (automatic / manual) and key-based transposition
- Scale snap (most effective on monophonic and vocal sources)
- Formant preservation / shift
- Glide (morphing between keys)
- LFO modulation
- Dry/wet, input/output gain, safety limiter
- Presets, A/B comparison, state persistence
- VST3 / CLAP bundles, macOS 12+ (Universal Binary)

### 2.2 Out of scope for v1.0 (future candidates)

- AAX / AU output (AU is a v1.1 candidate)
- MIDI pitch bend / CC modulation
- Harmony generation (additional voices such as +3rd / +5th)
- Full-mix processing that embeds stem separation
- Windows / Linux support
- Cloud preset sharing
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

- `Transpose`: −24 to +24 semitones (integer steps, automation supported)
- `Fine`: −100 to +100 cents (continuous)
- Total shift = Transpose + Fine/100 (a float in semitones)
- Apply 20 ms smoothing to changes to prevent clicks
- `Glide` sets the transition time for changes from 0 to 2000 ms

#### FR-2 Key-based transposition

- Set the input key to "automatic detection" or "manual"
- Set the output key (Target Key)
- Applied shift = Target Key − Source Key (in semitones)
  - Example: Source = A minor, Target = C minor → +3 semitones
- Choose the shortest offset within ±6 semitones
- If automatic detection is disabled or confidence is low, use the manual value
- When automatic detection is active, show the detected key and confidence
  (0–100%) in the UI

#### FR-3 Scale snap (monophonic / vocal sources)

- Pull each input note toward the scale notes of the Target Key
- `Snap Amount`: 0–100% (0 = no snap, 100 = snap fully to scale notes)
- `Retune Speed`: 0–200 ms (0 = immediate; larger values retune more gently)
- Scale presets: Major, Natural Minor, Harmonic Minor, Melodic Minor, Dorian,
  Phrygian, Lydian, Mixolydian, Locrian, Major Pentatonic, Minor Pentatonic,
  Blues, Chromatic
- In sections detected as polyphonic, fade the snap correction toward 0 based on
  the monophonicity confidence
- Show "most effective on monophonic sources" in the UI

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
  - Target: Transpose / Mix
- Automation
  - All primary parameters support host automation
  - Transpose / Target Key are applied with sample accuracy

#### FR-7 Mix and gain

- `Mix`: 0–100% (dry/wet, equal-power curve)
- `Input Gain` / `Output Gain`: −24 to +24 dB
- `Safety Limiter`: on/off (output −0.3 dBFS, internal to the plugin only)
- When pitch shift is 0 and Mix is 100%, guarantee bit-identical output to the dry
  path

### 4.2 Key detection

- Downmix the stereo input to mono and compute chroma features from the STFT
- Estimate tuning (A4 = 440 Hz ±50 cents) and correct the chroma
- Score 24 keys (12 roots × Major/Minor) by correlation with
  Krumhansl-Schmuckler profiles
- Apply exponential chroma smoothing with about a 1 second time constant
- Output: root (0–11), mode (Major / Minor), confidence (0–1)
- Update rate: every 2048 samples (about 43 ms at 48 kHz)
- During silence, hold the previous value and decay confidence

### 4.3 Presets / state

- Save presets as versioned JSON
- Persist plugin state through the host (VST3 state / CLAP state)
- Preset browser: save to and load from versioned JSON files
- A/B comparison: store and recall two parameter sets
- Undo: restore the previous snapshot before a preset recall or load (up to 100
  entries)
- Preset files carry a version field; unsupported versions are rejected with an
  error

### 4.4 UI

#### Layout (760 × 520 px, resizable)

- Main view: Key panel, a central pitch knob of up to 300 px, and a Quick panel
  with Scale Snap, Mix, Glide, and Formant controls.
- The knob readout is drawn inside the knob in semitones and cents.
- Top bar: preset save and load, bypass, and latency.
- Advanced (collapsed by default): Fine, Formant Shift, Retune, Scale, LFO,
  Quality, Input/Output, Limiter, and A/B controls.
- The control area scrolls when the window is smaller than the content.

Knob interaction:

- Drag vertically for coarse pitch changes.
- Shift-drag for cent-level changes.
- Scroll for fine steps; Shift-scroll for finer steps.
- Double-click to reset to 0.

#### UI requirements

- Custom rendering with egui. Target 60 fps
- The pitch knob is the primary control; it drives Transpose and Fine together
- When the editor has focus, pass transport and other non-navigation keys to the
  host unless a text field is active
- Do not use the default egui look; apply a custom theme (colors, corner radii,
  spacing, fonts)
- Draw the keyboard, circle of fifths, meters, and knobs from scratch with
  `Widget` implementations and `Painter`
- Use `egui_glow` callbacks for shader-based rendering (glow, spectrum) where useful
- Support HiDPI and remain intact at 150% / 200% scaling
- Highlight scale notes on the keyboard in two colors for Source and Target
- Optional circle-of-fifths display
- Double-click a value to type it directly
- Right-click menu with "Reset" and "Assign to automation"
- Keyboard operation: Tab to move focus, arrow keys for fine adjustment
- Distinguish Source and Target by shape as well as color for color-vision
  accessibility

---

## 5. Non-functional Requirements

### 5.1 Supported environment

| Item | Requirement |
|---|---|
| OS | macOS 12 Monterey or later |
| CPU | Apple Silicon (arm64) / Intel (x86_64) Universal Binary |
| Plugin formats | VST3 3.7.x, CLAP 1.x |
| DAW validation | Logic Pro, Ableton Live, Bitwig, Reaper, Cubase (VST3) |
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

- Plugin license: undecided (MIT or proprietary)
- Dependencies must be MIT / ISC / Apache-2.0 only
- Do not use GPL-licensed code (for example, Rubber Band)
- Code-sign and notarize releases
- Distribution format: `.pkg` or `.dmg` (bundling VST3 / CLAP)

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

### 6.6 Key detection details

- Input: downmixed signal (L+R)/2
- STFT: 4096 points, hop 2048, Hann window
- Chroma: 12 bins accumulated from spectral peaks (55 Hz–5 kHz)
- Refine peak frequencies with parabolic interpolation before mapping them to pitch
  classes
- Tuning: estimate the A4 offset from the strongest low-frequency peak (±50 cents)
  and compensate the chroma mapping
- F0: harmonic product spectrum (40–800 Hz, five harmonics) with parabolic
  refinement
- Monophonicity: spectral flatness and harmonic energy, smoothed over frames
- Key scoring: Pearson correlation against Krumhansl-Kessler Major / Minor profiles
  for all 24 keys
- Smoothing: exponential chroma smoothing (about 1 second time constant)
- Update rate: every 2048 samples (about 43 ms at 48 kHz)
- Output confidence: blend of the best correlation and its margin over the
  second-best score

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
| `transpose` | Transpose | −24 to +24 | 0 | st | 20 ms | Integer, sample-accurate |
| `fine` | Fine | −100 to +100 | 0 | ct | 20 ms | Continuous |
| `glide` | Glide | 0–2000 | 20 | ms | — | 0 = immediate |
| `formant_shift` | Formant | −12 to +12 | 0 | st | 20 ms | — |
| `formant_preserve` | Preserve | 0–100 | 100 | % | 20 ms | Most effective on monophonic sources |

### 7.3 Key

| ID | Display | Range | Default | Unit | Notes |
|---|---|---|---|---|---|
| `source_auto` | Source Auto | ON/OFF | OFF | — | OFF = manual |
| `source_root` | Source Root | C–B | C | — | Manual only |
| `source_mode` | Source Mode | Major / Minor | Minor | — | Manual only |
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
| `mod_target` | LFO Target | Transpose / Mix | Transpose | — | — |

---

## 8. Plugin Specification

### 8.1 Identifiers

| Item | Value |
|---|---|
| Plugin name | KeyMorph |
| Vendor | TBD |
| CLAP ID | `com.keymorph.keymorph` |
| VST3 UID | Generated from the vendor code (fixed) |
| Version | 0.1.0 (development), 1.0.0 (release) |

### 8.2 Buses / events

- Main input: stereo (mono also accepted)
- Main output: stereo
- Sidechain: none
- MIDI input: yes (pitch bend, CC, aftertouch)
- MIDI output: none
- Tempo information: used for LFO sync (CLAP / VST3 transport)

### 8.3 State

- Expose all parameters to the host and allow automation
- Serialize state as JSON (`serde`)
- Include a version field for future migration
- Include UI size and display settings in the state

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
| Plugin framework | nih-plug (VST3 / CLAP from one codebase) |
| UI | egui (`nih_plug_egui` / `egui_glow` backend) |
| Licensing | nih-plug: ISC / egui: MIT or Apache-2.0 |

Reasons:

1. Produce VST3 and CLAP from one codebase
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
| `serde` / `serde_json` | State persistence | MIT / Apache-2.0 |
| `rfd` | File dialogs (presets) | MIT |

- The pitch shifter uses Signalsmith Stretch (MIT)
- Build requires a C++ compiler and libclang (for bindgen)
- Do not use the `rubberband` crate because it is GPL

---

## 10. Architecture

### 10.1 Threading model

```
┌──────────────┐    atomic params    ┌──────────────────┐
│  UI thread   │ ──────────────────► │  Audio thread    │
│  (egui)      │ ◄────────────────── │  (process)       │
└──────────────┘   meters / key info └──────────────────┘
        │                                     │
        │ preset save/load                    │ lock-free queue
        ▼                                     ▼
┌──────────────┐                     ┌──────────────────┐
│  Disk / Host │                     │  Worker thread   │
│  state       │                     │  key detection   │
└──────────────┘                     └──────────────────┘
```

- Share parameters through `AtomicFloat` / `AtomicEnum`
- Offload key detection to a worker thread (pass audio through a ring buffer)
- Poll state from the UI at 30–60 fps (no locks)
- On the audio thread, prohibit `Vec` reallocation, `Mutex`, and file I/O

### 10.2 Directory layout

```
KeyMorph/
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
cargo xtask bundle keymorph --release
# → target/bundled/KeyMorph.vst3
# → target/bundled/KeyMorph.clap

# Universal release bundles (arm64 + x86_64)
cargo xtask bundle-universal keymorph --release
```

### 11.2 Release

1. Pass `cargo test` / `cargo clippy`
2. `cargo xtask bundle-universal keymorph --release`
3. Code-sign (Developer ID Application)
4. Notarize with `notarytool`
5. Create a `.dmg` and attach the ticket with `stapler`
6. Validate with pluginval (VST3) and clap-validator

### 11.3 CI

- GitHub Actions (macOS runner)
- `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test`
- Automate bundling and notarization on tag pushes (keep certificates in Secrets)

---

## 12. Testing / Acceptance Criteria

### 12.1 Unit tests

- Pitch detection: within ±5 cents on a 40–1000 Hz sine sweep
- Key detection: at least 90% correct on synthetic chords and real samples for all
  24 keys
- Scale snap: a detuned note is corrected to the nearest scale degree
- FFT / iSTFT: covered by Signalsmith Stretch
- Engine latency: impulse peak at the reported latency (±1 sample)
- Pitch shift: 440 Hz → 880 Hz within 10 Hz after the latency
- Formant control: formant shift moves spectral energy in the expected direction
- Monophonic detection: a sine is classified as monophonic and reports its F0
- Latency: difference between reported and measured values within 1 sample, using
  an impulse response; all quality presets report 6720 samples at 48 kHz
- State: all parameters match after save → load
- Presets: a captured snapshot round-trips through JSON without loss
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

- pluginval (level 10)
- clap-validator
- Manual DAW checks (Logic / Live / Bitwig / Reaper)

### 12.4 Known validation exceptions

- CLAP state load: nih-plug's CLAP wrapper does not request a parameter value rescan
  after loading state through the host state extension. VST3 is unaffected. This is
  an upstream nih-plug issue.
- CLAP malformed state: nih-plug's CLAP wrapper trusts the length prefix in the
  state stream, so malformed state can trigger a large allocation. Hosts write
  valid state, so this only affects fuzzing and corrupted files. This is an
  upstream nih-plug issue.
- VST3 passes pluginval at strictness level 10, including editor and parameter
  tests.

### 12.5 Acceptance criteria (v1.0)

- [ ] Pitch shifting works on monophonic, polyphonic, and mixed material
- [ ] Key detection and automatic transposition work
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
| M0 | Project skeleton, VST3 / CLAP output, bypass + gain | 1 week |
| M1 | Signalsmith Stretch integration, parameters, latency reporting | 2–3 weeks |
| M2 | Formant mapping, key detection, scale snap | 2–3 weeks |
| M3 | Key detection, key UI, scale snap | 2 weeks |
| M4 | LFO, limiter, presets, A/B, UI polish | 1–2 weeks |
| M5 | Validation, optimization, Universal Binary, signing / notarization, release | 1–2 weeks |

---

## 14. Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| F0 misdetection in monophonic passages | Medium | Confidence gating, smoothing, manual source key |
| Formant artifacts at large shifts | Medium | Preserve slider, Signalsmith formant compensation |
| CPU load on mixed material | Medium | Quality presets, High only when needed |
| Relative-key misdetection (Am ↔ C) | Medium | Show confidence, allow manual override, tune smoothing |
| Latency vs. quality trade-off | Medium | Fixed 140 ms reported latency at 48 kHz, host compensation, no zero-latency mode |
| Build complexity from Signalsmith FFI | Low | Pinned crate, prebuilt buffers, CI build check |
| Notarization / signing operational burden | Low | Automate in CI, manage certificates in Secrets |
| CLAP state-load rescan (upstream nih-plug) | Medium | VST3 validated; document and track an upstream fix |
| CLAP malformed-state allocation (upstream nih-plug) | Low | Document; valid host state is unaffected |
| Out-of-tune feel from scale snap | Medium | Separate snap amount from retune speed; make both preset-adjustable |

---

## 15. Open Questions

- [ ] License (MIT / proprietary)
- [ ] Vendor name and VST3 UID
- [ ] Price / distribution channel (direct / App Store / free)
- [ ] Include AU support in v1.1?
- [ ] Include harmony generation in v1.0?
- [ ] Visual tone of the UI (colors, fonts, texture)
- [ ] Custom scale editor (per-note on/off)
- [ ] MIDI pitch bend / CC modulation
- [ ] Is cloud preset sharing needed?
- [ ] How far to support Intel Macs (is a Universal Binary mandatory)?

---

## 16. References

- nih-plug: https://github.com/robbert-vdh/nih-plug
- CLAP: https://github.com/free-audio/clap
- VST3 SDK: https://github.com/steinbergmedia/vst3sdk
- Signalsmith Stretch: https://github.com/Signalsmith-Audio/signalsmith-stretch
- Signalsmith Linear: https://github.com/Signalsmith-Audio/linear
- Krumhansl-Schmuckler key profiles: Krumhansl, C. L. (1990), *Cognitive Foundations of Musical Pitch*
- Harmonic product spectrum: Schroeder (1968), *Period histogram and product spectrum: New methods for fundamental-frequency measurement*
