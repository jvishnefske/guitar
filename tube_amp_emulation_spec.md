# Vintage Tube Amp Emulation DSP

## Software Specification

**Version:** 0.1.0  
**Target Platform:** Embedded Rust (`no_std`)  
**Author:** jvishnefske  
**Date:** January 2026

---

## 1. Overview

### 1.1 Purpose

This specification defines a real-time digital signal processing system that emulates the tonal characteristics of vintage guitar tube amplifiers. The system processes electric guitar input and outputs processed audio via Bluetooth, suitable for integration into a custom-built electric guitar with onboard DSP.

### 1.2 Design Goals

- Authentic reproduction of vintage tube amp nonlinear behavior
- Low-latency processing suitable for live performance (target < 10ms)
- Memory-efficient implementation for embedded deployment
- Configurable amp voicings (Fender, Marshall, Vox archetypes)
- Safe, verifiable Rust implementation with `no_std` compatibility

### 1.3 Scope

The system models preamp gain stages, tone shaping networks, power amp compression, and speaker cabinet response. It does not model effects (reverb, tremolo, vibrato) which may be implemented as separate modules.

---

## 2. System Architecture

### 2.1 Signal Flow

```
┌─────────┐   ┌──────────┐   ┌─────────────┐   ┌────────────┐
│  Input  │──▶│  Input   │──▶│   Preamp    │──▶│   Tone     │
│  Stage  │   │  Filter  │   │   Stages    │   │   Stack    │
└─────────┘   └──────────┘   └─────────────┘   └────────────┘
                                                      │
┌─────────┐   ┌──────────┐   ┌─────────────┐         │
│ Output  │◀──│ Cabinet  │◀──│  Power Amp  │◀────────┘
│  Stage  │   │    IR    │   │   Model     │
└─────────┘   └──────────┘   └─────────────┘
```

### 2.2 Processing Blocks

| Block | Function | Computational Load |
|-------|----------|-------------------|
| Input Stage | Level conditioning, DC blocking | Low |
| Input Filter | Guitar cable/pickup resonance | Low |
| Preamp Stages | Triode gain stage emulation (1-4 stages) | Medium-High |
| Tone Stack | Passive EQ network modeling | Low |
| Power Amp Model | Compression, sag, transformer coloration | Medium |
| Cabinet IR | Convolution with speaker impulse response | High |
| Output Stage | Level normalization, soft limiting | Low |

### 2.3 Sample Rate and Buffer Size

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Sample Rate | 48,000 Hz | Standard audio rate, sufficient bandwidth |
| Buffer Size | 64-128 samples | Balance latency vs. efficiency |
| Bit Depth | 32-bit float (internal) | Headroom for nonlinear processing |
| Output Format | 16-bit or 24-bit PCM | Bluetooth codec compatibility |

---

## 3. Processing Block Specifications

### 3.1 Input Stage

**Purpose:** Normalize input level and remove DC offset.

**Implementation:**
- DC blocking high-pass filter: 1st order IIR, fc = 10 Hz
- Input gain: 0 dB to +20 dB, user configurable

**Transfer Function (DC Block):**
```
H(z) = (1 - z⁻¹) / (1 - α·z⁻¹)
where α = 1 - (2π·fc / fs)
```

### 3.2 Input Filter

**Purpose:** Model guitar pickup and cable resonance characteristics.

**Implementation:**
- 2nd order resonant low-pass filter (biquad)
- Resonant frequency: 2-5 kHz (configurable)
- Q factor: 0.5-2.0 (configurable)

**Parameters:**
| Parameter | Range | Default | Unit |
|-----------|-------|---------|------|
| `pickup_freq` | 2000-5000 | 3500 | Hz |
| `pickup_q` | 0.5-2.0 | 1.0 | - |

### 3.3 Preamp Stages

**Purpose:** Model triode vacuum tube gain stages with characteristic nonlinear distortion.

#### 3.3.1 Single Stage Model

Each preamp stage consists of:
1. Coupling capacitor high-pass filter
2. Grid conduction limiter (asymmetric)
3. Tube transfer function (waveshaper)
4. Plate load frequency shaping

**Coupling Capacitor:**
- 1st order high-pass, fc = 7-15 Hz (models interstage coupling)
- Frequency shifts under bias from grid conduction

**Grid Conduction Model:**
```rust
fn grid_conduct(x: f32, threshold: f32) -> f32 {
    if x > threshold {
        threshold + (x - threshold) * 0.1  // soft limit positive
    } else {
        x
    }
}
```

**Tube Transfer Function:**

The core nonlinearity uses an asymmetric waveshaper:

```rust
fn triode_waveshape(x: f32, drive: f32, asymmetry: f32) -> f32 {
    let driven = x * drive;
    let symmetric = tanh_approx(driven);
    let asymmetric = tanh_approx(driven).powi(2) * asymmetry;
    symmetric + asymmetric
}
```

Where `tanh_approx` is a computationally efficient approximation:

```rust
fn tanh_approx(x: f32) -> f32 {
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}
```

**Stage Parameters:**
| Parameter | Range | Default | Unit |
|-----------|-------|---------|------|
| `stage_gain` | 1.0-100.0 | 30.0 | - |
| `asymmetry` | 0.0-0.5 | 0.15 | - |
| `coupling_fc` | 7-50 | 15 | Hz |
| `bias_shift` | 0.0-1.0 | 0.2 | - |

#### 3.3.2 Stage Cascade

The preamp consists of 1-4 cascaded stages. Each subsequent stage operates on the output of the previous, accumulating harmonic content.

**Cascade Configuration by Amp Type:**
| Amp Archetype | Stages | Gain Distribution |
|---------------|--------|-------------------|
| Clean (Fender) | 2 | [25, 20] |
| Crunch (Marshall) | 3 | [35, 30, 25] |
| High Gain | 4 | [40, 35, 30, 25] |

### 3.4 Tone Stack

**Purpose:** Model passive tone control networks found in guitar amplifiers.

#### 3.4.1 Topology

Three common topologies with distinct voicings:

**Fender (TMB):** Mid-scooped, bright character
**Marshall (TMB):** Mid-forward, aggressive character  
**Vox (Cut):** Single cut control, chimey character

#### 3.4.2 Implementation

Tone stacks are implemented as cascaded biquad filters derived from circuit analysis. Coefficients are computed from potentiometer positions.

**Fender Tone Stack Transfer Function:**

Modeled as 3 biquad sections:
1. Bass shelf
2. Mid scoop/boost
3. Treble shelf

**Parameters:**
| Parameter | Range | Default |
|-----------|-------|---------|
| `bass` | 0.0-1.0 | 0.5 |
| `mid` | 0.0-1.0 | 0.5 |
| `treble` | 0.0-1.0 | 0.5 |

**Coefficient Calculation:**

Coefficients are pre-computed for discrete pot positions (e.g., 21 steps per control) and stored in lookup tables, or interpolated at runtime.

```rust
struct ToneStackCoeffs {
    bass_biquad: BiquadCoeffs,
    mid_biquad: BiquadCoeffs,
    treble_biquad: BiquadCoeffs,
}

fn compute_fender_stack(bass: f32, mid: f32, treble: f32) -> ToneStackCoeffs {
    // Component values (Fender Bassman reference)
    const R1: f32 = 250_000.0;  // Treble pot
    const R2: f32 = 1_000_000.0; // Bass pot
    const R3: f32 = 25_000.0;    // Mid pot
    const C1: f32 = 250e-12;     // 250pF
    const C2: f32 = 20e-9;       // 20nF
    const C3: f32 = 20e-9;       // 20nF
    // ... derive biquad coefficients from component values and pot positions
}
```

### 3.5 Power Amp Model

**Purpose:** Model power tube compression, transformer saturation, and supply sag.

#### 3.5.1 Components

**Push-Pull Crossover:**
Models class AB crossover distortion at low signal levels.

```rust
fn crossover_model(x: f32, dead_zone: f32) -> f32 {
    if x.abs() < dead_zone {
        x * (x.abs() / dead_zone)  // smooth through dead zone
    } else {
        x
    }
}
```

**Power Supply Sag:**
Models B+ voltage drop under heavy load, causing compression and "bloom."

```rust
struct SagFilter {
    envelope: f32,
    attack: f32,   // slow attack (10-50ms)
    release: f32,  // slow release (100-500ms)
}

impl SagFilter {
    fn process(&mut self, x: f32) -> f32 {
        let rect = x.abs();
        if rect > self.envelope {
            self.envelope += self.attack * (rect - self.envelope);
        } else {
            self.envelope += self.release * (rect - self.envelope);
        }
        let sag_amount = 1.0 - (self.envelope * 0.3).min(0.3);
        x * sag_amount
    }
}
```

**Output Transformer:**
Models high-frequency rolloff and resonance of output transformer.

- 2nd order low-pass filter, fc = 5-8 kHz
- Gentle resonant peak at cutoff

**Parameters:**
| Parameter | Range | Default | Unit |
|-----------|-------|---------|------|
| `sag_depth` | 0.0-1.0 | 0.3 | - |
| `sag_attack` | 10-100 | 30 | ms |
| `sag_release` | 50-500 | 200 | ms |
| `transformer_fc` | 4000-10000 | 6000 | Hz |

### 3.6 Cabinet Impulse Response

**Purpose:** Convolve signal with speaker cabinet impulse response for realistic speaker coloration.

#### 3.6.1 Implementation

**Convolution Method:** 
- Short IRs (≤512 samples): Direct time-domain convolution
- Longer IRs: Partitioned overlap-add FFT convolution

**Recommended Approach:**
Use 256-512 sample IRs (5-10ms at 48kHz) for minimal latency while capturing essential cabinet character.

```rust
struct CabinetIR {
    ir_samples: [f32; 512],
    delay_line: [f32; 512],
    write_pos: usize,
}

impl CabinetIR {
    fn process(&mut self, x: f32) -> f32 {
        self.delay_line[self.write_pos] = x;
        let mut sum = 0.0;
        for i in 0..512 {
            let read_pos = (self.write_pos + 512 - i) % 512;
            sum += self.delay_line[read_pos] * self.ir_samples[i];
        }
        self.write_pos = (self.write_pos + 1) % 512;
        sum
    }
}
```

**IR Storage:**
IRs stored as fixed-point or compressed format in flash memory. Multiple IRs selectable at runtime.

**Preset IRs:**
| IR Name | Character | Length |
|---------|-----------|--------|
| `1x12_american` | Bright, focused | 256 samples |
| `2x12_british` | Warm, midrange | 384 samples |
| `4x12_heavy` | Deep, full | 512 samples |
| `1x10_vintage` | Thin, nasal | 256 samples |

### 3.7 Output Stage

**Purpose:** Final level control and safety limiting.

**Implementation:**
- Master volume control (0 to -60 dB)
- Soft clipper to prevent digital overs
- Optional output high-pass (subsonic filter)

```rust
fn output_soft_clip(x: f32, ceiling: f32) -> f32 {
    let threshold = ceiling * 0.8;
    if x.abs() > threshold {
        let excess = x.abs() - threshold;
        let compressed = threshold + excess / (1.0 + excess / (ceiling - threshold));
        compressed.copysign(x)
    } else {
        x
    }
}
```

---

## 4. Preset System

### 4.1 Amp Presets

Pre-configured parameter sets modeling specific amp characteristics.

| Preset | Archetype | Stages | Tone Stack | Character |
|--------|-----------|--------|------------|-----------|
| `clean_twin` | Fender Twin | 2 | Fender | Bright, headroom |
| `tweed_deluxe` | Fender Deluxe | 2 | Fender | Warm breakup |
| `plexi_crunch` | Marshall JTM45 | 3 | Marshall | Classic rock |
| `brit_high` | Marshall JCM800 | 3 | Marshall | Hard rock |
| `ac30_chime` | Vox AC30 | 3 | Vox | Chimey, jangly |
| `recto_heavy` | Modern High Gain | 4 | Scooped | Metal, djent |

### 4.2 Preset Data Structure

```rust
#[derive(Clone, Copy)]
struct AmpPreset {
    name: &'static str,
    
    // Input
    input_gain_db: f32,
    pickup_freq: f32,
    pickup_q: f32,
    
    // Preamp
    num_stages: u8,
    stage_gains: [f32; 4],
    stage_asymmetry: [f32; 4],
    coupling_fc: [f32; 4],
    
    // Tone stack
    tone_stack_type: ToneStackType,
    bass: f32,
    mid: f32,
    treble: f32,
    
    // Power amp
    sag_depth: f32,
    sag_attack_ms: f32,
    sag_release_ms: f32,
    transformer_fc: f32,
    
    // Cabinet
    cabinet_ir_index: u8,
    
    // Output
    master_volume_db: f32,
}

#[derive(Clone, Copy)]
enum ToneStackType {
    Fender,
    Marshall,
    Vox,
    Bypassed,
}
```

---

## 5. Control Interface

### 5.1 Real-Time Parameters

Parameters adjustable during performance without audio glitches:

| Parameter | Control | Smoothing |
|-----------|---------|-----------|
| Input Gain | Knob/MIDI | 10ms slew |
| Drive (per stage) | Knob/MIDI | 10ms slew |
| Bass/Mid/Treble | Knob/MIDI | 20ms slew |
| Master Volume | Knob/MIDI | 10ms slew |
| Preset Selection | Switch/MIDI | Crossfade 50ms |

### 5.2 Configuration Parameters

Parameters set at initialization or requiring buffer flush:

- Sample rate
- Buffer size
- Cabinet IR selection
- Number of preamp stages

### 5.3 Bluetooth Control (Optional)

MIDI over BLE for wireless parameter control:
- CC messages mapped to real-time parameters
- Program change for preset selection
- SysEx for preset upload/download

---

## 6. Implementation Requirements

### 6.1 Rust Crate Structure

```
tube_amp_dsp/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── input.rs        # Input stage, DC block
│   ├── preamp.rs       # Triode stages, waveshaping
│   ├── tonestack.rs    # Tone stack models
│   ├── poweramp.rs     # Sag, compression, transformer
│   ├── cabinet.rs      # IR convolution
│   ├── output.rs       # Limiting, volume
│   ├── biquad.rs       # Biquad filter implementation
│   ├── preset.rs       # Preset definitions
│   └── dsp_math.rs     # tanh approx, utilities
└── tests/
    └── integration.rs
```

### 6.2 Dependencies

```toml
[dependencies]
# No std support
libm = "0.2"           # Math functions for no_std

[dev-dependencies]
approx = "0.5"         # Float comparison in tests
```

### 6.3 Safety Requirements

- No `unsafe` blocks except where required for hardware interface
- All array accesses bounds-checked or proven safe
- No heap allocation (`#![no_std]` compatible)
- No floating-point exceptions (handle NaN/Inf)
- All parameters validated at API boundary

### 6.4 Verification

**Unit Tests:**
- Each processing block tested in isolation
- Known input → expected output for deterministic functions
- Coefficient calculations verified against reference

**Integration Tests:**
- Full signal chain with known test signals
- Impulse response capture for linear sections
- THD measurement for nonlinear sections

**Static Analysis:**
- `cargo clippy` with pedantic lints
- `cargo miri` for undefined behavior detection
- Stack usage analysis for embedded target

---

## 7. Performance Targets

### 7.1 Computational Budget

**Target Platform:** ARM Cortex-M4F @ 168 MHz (STM32F4 class)

| Block | Cycles/Sample | % Budget |
|-------|---------------|----------|
| Input Stage | 50 | 1.4% |
| Preamp (4 stages) | 800 | 22.9% |
| Tone Stack | 200 | 5.7% |
| Power Amp | 300 | 8.6% |
| Cabinet IR (256 tap) | 1500 | 42.9% |
| Output Stage | 50 | 1.4% |
| **Total** | **2900** | **82.9%** |

Budget assumes 48kHz sample rate → 3500 cycles/sample available.

### 7.2 Memory Budget

| Resource | Allocation |
|----------|------------|
| Code (Flash) | ≤ 64 KB |
| Preset Data (Flash) | ≤ 16 KB |
| Cabinet IRs (Flash) | ≤ 32 KB |
| Working RAM | ≤ 8 KB |
| Stack | ≤ 2 KB |

### 7.3 Latency Budget

| Contribution | Latency |
|--------------|---------|
| Input buffer | 2.67 ms (128 samples) |
| Processing | < 0.1 ms |
| Output buffer | 2.67 ms (128 samples) |
| Bluetooth codec | ~3-5 ms |
| **Total** | **~9-11 ms** |

---

## 8. Future Extensions

### 8.1 Planned Features

- Noise gate (input)
- Presence/resonance controls (power amp feedback)
- Stereo cabinet simulation
- Additional amp models
- User IR loading via Bluetooth

### 8.2 Research Areas

- Machine learning waveshaper training from amp measurements
- Wave digital filter implementation for higher fidelity
- Adaptive latency based on buffer utilization

---

## Appendix A: Reference Circuits

### A.1 12AX7 Triode Operating Point

| Parameter | Value |
|-----------|-------|
| Plate Voltage | 250V |
| Plate Current | 1.2mA |
| Grid Bias | -2V |
| Amplification Factor (μ) | 100 |
| Plate Resistance (rp) | 62.5 kΩ |
| Transconductance (gm) | 1.6 mA/V |

### A.2 Fender Bassman Tone Stack Components

| Component | Value |
|-----------|-------|
| R_treble | 250 kΩ pot |
| R_bass | 1 MΩ pot |
| R_mid | 25 kΩ pot |
| R_slope | 56 kΩ |
| C1 | 250 pF |
| C2 | 20 nF |
| C3 | 20 nF |

---

## Appendix B: Glossary

| Term | Definition |
|------|------------|
| **Biquad** | Second-order IIR filter section |
| **IR** | Impulse Response |
| **Sag** | Power supply voltage drop under load |
| **TMB** | Treble-Mid-Bass tone control arrangement |
| **Waveshaper** | Nonlinear function applied sample-by-sample |

---

*End of Specification*
