# Guitar Amp DSP System - MVP Functional Requirements

**Version:** 0.1.0
**Platform:** ESP32-S3
**References:** `tube_amp_emulation_spec.md`, `README.md`

---

## A. Platform Infrastructure

- [ ] **A1** ESP32-S3 crate structure with `no_std` DSP core
- [ ] **A2** Cargo.toml with dependencies (`libm`, `embassy`, `esp-hal`)
- [ ] **A3** Module exports in lib.rs
- [ ] **A4** 64KB heap allocator initialization

**Verification:** `cargo build --target xtensa-esp32s3-espidf` succeeds

---

## B. USB Audio Subsystem

- [ ] **B1** USB Audio Class device implementation
- [ ] **B2** Device appears as "USB Speaker with DSP" to host
- [ ] **B3** 48kHz sample rate, 16-bit depth, stereo
- [ ] **B4** 4x1ms frame buffering (4ms latency contribution)
- [ ] **B5** SPSC queue for audio buffer transfer to DSP task

**Verification:** Host OS recognizes device; audio streams without dropouts

---

## C. Bluetooth Subsystem

- [ ] **C1** A2DP Source role streaming to headphones
- [ ] **C2** SBC encoder integration
- [ ] **C3** Device discovery command via BLE
- [ ] **C4** Pairing and connection management
- [ ] **C5** Auto-reconnect with 30s timeout

**Verification:** Audio plays through paired Bluetooth headphones

---

## D. BLE Control Interface

- [ ] **D1** GATT service UUID `12345678-1234-5678-1234-56789abcdef0`
- [ ] **D2** Control characteristic (Write) for commands
- [ ] **D3** Status characteristic (Read/Notify) for device state
- [ ] **D4** Command serialization via postcard/serde
- [ ] **D5** Real-time parameter updates without audio glitch
- [ ] **D6** Status notifications to phone app

**Verification:** Phone app can send commands and receive status updates

---

## E. DSP Signal Chain

### E1. Input Stage (spec 3.1)
- [ ] **E1.1** DC blocking high-pass filter: 1st order IIR, fc=10Hz
- [ ] **E1.2** Input gain: 0dB to +20dB, configurable

**Verification:** DC offset removed; gain adjustable without clipping

### E2. Input Filter (spec 3.2)
- [x] **E2.1** 2nd order resonant low-pass filter (biquad)
- [x] **E2.2** Resonant frequency: 2-5kHz configurable
- [x] **E2.3** Q factor: 0.5-2.0 configurable

**Verification:** Frequency response matches pickup resonance curve

### E3. Preamp Stages (spec 3.3)
- [x] **E3.1** Support 1-4 cascaded triode stages
- [x] **E3.2** Coupling capacitor HPF per stage (fc=7-50Hz)
- [x] **E3.3** Grid conduction limiter (asymmetric soft clip)
- [x] **E3.4** Triode waveshaper: asymmetric tanh approximation
- [x] **E3.5** Per-stage gain: 1.0-100.0
- [x] **E3.6** Per-stage asymmetry: 0.0-0.5

**Verification:** Harmonic distortion increases with drive; asymmetry audible; `cargo test preamp` passes (29/29 tests)

### E4. Tone Stack (spec 3.4)
- [x] **E4.1** Fender topology (mid-scooped)
- [x] **E4.2** Marshall topology (mid-forward)
- [x] **E4.3** Vox topology (cut control)
- [x] **E4.4** Bass/Mid/Treble controls: 0.0-1.0 each
- [x] **E4.5** Implemented as cascaded biquad sections

**Verification:** EQ response matches reference curves per topology (30 unit tests pass)

### E5. Power Amp Model (spec 3.5)
- [x] **E5.1** Push-pull crossover distortion model
- [x] **E5.2** Power supply sag filter (attack 10-100ms, release 50-500ms)
- [x] **E5.3** Sag depth: 0.0-1.0
- [x] **E5.4** Output transformer LPF: fc=4-10kHz

**Verification:** Compression bloom audible on sustained notes; `cargo test poweramp` passes (19/19 tests)

### E6. Cabinet IR (spec 3.6)
- [x] **E6.1** Time-domain convolution engine
- [x] **E6.2** Support 256-512 sample IRs
- [x] **E6.3** Circular delay line implementation
- [x] **E6.4** IR storage in flash, selectable at runtime

**Verification:** Cabinet coloration applied; different IRs sound distinct; `cargo test cabinet` passes (22/22 tests)

### E7. Output Stage (spec 3.7)
- [x] **E7.1** Master volume: 0 to -60dB
- [x] **E7.2** Soft clipper to prevent digital overs
- [x] **E7.3** Output ceiling at 0.8 threshold

**Verification:** No digital clipping; volume control smooth; `cargo test output` passes (22/22 tests)

---

## F. Filter Engine (fir_filter.rs)

- [x] **F1** FIR filter with up to 128 taps
- [x] **F2** FilterPreset enum: Bypass, LowPass, HighPass, BandPass, Parametric, Custom
- [x] **F3** Windowed sinc design for LP/HP/BP
- [x] **F4** Atomic parameter updates (no audio glitch)
- [x] **F5** Circular delay line implementation

**Verification:** Existing unit tests pass

---

## F2. Biquad Filter Engine (biquad.rs)

- [x] **F2.1** Direct Form II Transposed implementation
- [x] **F2.2** Low-pass filter with configurable fc and Q
- [x] **F2.3** High-pass filter with configurable fc and Q
- [x] **F2.4** Band-pass filter with configurable fc and Q
- [x] **F2.5** Peaking EQ filter with configurable fc, gain_db, and Q
- [x] **F2.6** Low shelf filter with configurable fc and gain_db
- [x] **F2.7** High shelf filter with configurable fc and gain_db
- [x] **F2.8** Single sample processing (process_sample)
- [x] **F2.9** In-place buffer processing (process_buffer)
- [x] **F2.10** State reset method (reset)
- [x] **F2.11** `no_std` compatible using libm for math
- [x] **F2.12** Comprehensive unit tests (24 tests)

**Verification:** `cargo test biquad` passes (24/24 tests)

**Usage:** Foundation for E1 (DC blocking), E2 (pickup resonance), E4 (tone stack), E5 (transformer LPF)

---

## G. Preset System (spec 4)

### G1. Amp Presets
- [x] **G1.1** `clean_twin` - Fender Twin (2 stages, Fender stack)
- [x] **G1.2** `tweed_deluxe` - Fender Deluxe (2 stages, warm breakup)
- [x] **G1.3** `plexi_crunch` - Marshall JTM45 (3 stages, Marshall stack)
- [x] **G1.4** `brit_high` - Marshall JCM800 (3 stages, hard rock)
- [x] **G1.5** `ac30_chime` - Vox AC30 (3 stages, Vox stack)
- [x] **G1.6** `recto_heavy` - Modern high gain (4 stages, scooped)

### G2. Cabinet IRs
- [x] **G2.1** `1x12_american` - Bright, focused (256 samples)
- [x] **G2.2** `2x12_british` - Warm, midrange (384 samples)
- [x] **G2.3** `4x12_heavy` - Deep, full (512 samples)
- [x] **G2.4** `1x10_vintage` - Thin, nasal (256 samples)

### G3. Preset Data Structure
- [x] **G3.1** AmpPreset struct with all parameters
- [x] **G3.2** ToneStackType enum (Fender, Marshall, Vox, Bypassed)
- [ ] **G3.3** Preset crossfade on switch (50ms)

**Verification:** Each preset produces characteristic sound

---

## H. Non-Functional Requirements

- [ ] **H1** No `unsafe` blocks in DSP hot path
- [ ] **H2** All array accesses bounds-checked or proven safe
- [ ] **H3** No heap allocation in audio processing loop
- [ ] **H4** End-to-end latency ≤50ms
- [ ] **H5** CPU usage ≤30% at 240MHz (ESP32-S3)
- [ ] **H6** Panic triggers safe device restart
- [ ] **H7** Unit tests for each DSP block
- [ ] **H8** `cargo clippy` clean with pedantic lints

**Verification:** Latency measurement <50ms; no panics under load

---

## Traceability Matrix

| Req ID | Requirement | Source |
|--------|-------------|--------|
| A1-A4 | Platform Infrastructure | README.md |
| B1-B5 | USB Audio | README.md §USB Audio |
| C1-C5 | Bluetooth A2DP | README.md §Bluetooth |
| D1-D6 | BLE Control | README.md §BLE Protocol |
| E1 | Input Stage | tube_amp_emulation_spec.md §3.1 |
| E2 | Input Filter | tube_amp_emulation_spec.md §3.2 |
| E3 | Preamp Stages | tube_amp_emulation_spec.md §3.3 |
| E4 | Tone Stack | tube_amp_emulation_spec.md §3.4 |
| E5 | Power Amp | tube_amp_emulation_spec.md §3.5 |
| E6 | Cabinet IR | tube_amp_emulation_spec.md §3.6 |
| E7 | Output Stage | tube_amp_emulation_spec.md §3.7 |
| F1-F5 | FIR Filter Engine | fir_filter.rs (implemented) |
| F2.1-F2.12 | Biquad Filter Engine | biquad.rs (implemented) |
| G1-G3 | Preset System | tube_amp_emulation_spec.md §4 |
| H1-H8 | Non-Functional | README.md, tube_amp_emulation_spec.md §6-7 |

---

## Implementation Priority

**Phase 1 - Core DSP (MVP)**
1. E1 Input Stage
2. E3 Preamp Stages (single stage first)
3. E7 Output Stage
4. G3 Preset structure

**Phase 2 - Tone Shaping**
1. E2 Input Filter
2. E4 Tone Stack
3. E5 Power Amp
4. E6 Cabinet IR

**Phase 3 - Platform Integration**
1. B1-B5 USB Audio
2. C1-C5 Bluetooth
3. D1-D6 BLE Control

**Phase 4 - Presets & Polish**
1. G1 Amp Presets
2. G2 Cabinet IRs
3. H7-H8 Testing & Linting
