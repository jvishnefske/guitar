//! Preset Module
//!
//! Pre-configured parameter sets modeling specific amp characteristics.
//!
//! # Overview
//!
//! This module provides six carefully tuned amp presets that model classic
//! amplifier characteristics. Each preset configures all parameters of the
//! signal chain to achieve a specific tonal character.
//!
//! # Available Presets
//!
//! | Preset | Archetype | Character |
//! |--------|-----------|-----------|
//! | [`CLEAN_TWIN`] | Fender Twin | Bright, clean headroom |
//! | [`TWEED_DELUXE`] | Fender Deluxe | Warm, musical breakup |
//! | [`PLEXI_CRUNCH`] | Marshall JTM45 | Classic rock crunch |
//! | [`BRIT_HIGH`] | Marshall JCM800 | Hard rock aggression |
//! | [`AC30_CHIME`] | Vox AC30 | Chimey, jangly |
//! | [`RECTO_HEAVY`] | Modern High Gain | Metal, djent |
//!
//! # Usage
//!
//! ```ignore
//! use crate::preset::{all_presets, preset_by_name, CLEAN_TWIN};
//!
//! // Access a preset directly
//! let preset = CLEAN_TWIN;
//! println!("Using preset: {}", preset.name);
//!
//! // Find a preset by name
//! if let Some(preset) = preset_by_name("plexi_crunch") {
//!     // Configure signal chain with preset parameters
//! }
//!
//! // Iterate over all presets
//! for preset in all_presets() {
//!     println!("{}: {} stages", preset.name, preset.num_stages);
//! }
//! ```
//!
//! # Design Notes
//!
//! Presets are compile-time constants (`const`) for zero runtime overhead.
//! The [`AmpPreset`] struct uses `Copy` semantics for efficient parameter passing.
//!
//! Preset crossfade (G3.3: 50ms) is handled by the signal chain module, not here.
//!
//! # References
//!
//! - tube_amp_emulation_spec.md Section 4
//! - design.md requirements G1.1-G1.6, G3.1-G3.2

use crate::tonestack::ToneStackType;

/// Complete amp preset with all configurable parameters.
///
/// This struct captures every tunable parameter of the amp model signal chain,
/// allowing complete characterization of an amplifier's tonal signature.
///
/// # Parameter Groups
///
/// - **Input stage**: Initial gain and pickup simulation
/// - **Preamp**: Cascaded tube stages with coupling and waveshaping
/// - **Tone stack**: EQ topology and control settings
/// - **Power amp**: Compression, sag, and transformer coloration
/// - **Cabinet**: IR selection for speaker simulation
/// - **Output**: Master volume and final limiting
///
/// # Example
///
/// ```ignore
/// let preset = AmpPreset {
///     name: "custom",
///     input_gain_db: 10.0,
///     pickup_freq: 3000.0,
///     pickup_q: 1.0,
///     num_stages: 2,
///     stage_gains: [40.0, 30.0, 0.0, 0.0],
///     stage_asymmetry: [0.1, 0.15, 0.0, 0.0],
///     coupling_fc: [10.0, 15.0, 0.0, 0.0],
///     grid_threshold: [0.7, 0.6, 0.0, 0.0],
///     tone_stack_type: ToneStackType::Fender,
///     bass: 0.5,
///     mid: 0.5,
///     treble: 0.6,
///     crossover_amount: 0.1,
///     sag_depth: 0.2,
///     sag_attack_ms: 20.0,
///     sag_release_ms: 150.0,
///     transformer_fc: 6000.0,
///     cabinet_ir_index: 0,
///     master_volume_db: -6.0,
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmpPreset {
    /// Preset identifier name.
    ///
    /// Used for lookup via [`preset_by_name`] and display purposes.
    pub name: &'static str,

    // -------------------------------------------------------------------------
    // Input Stage
    // -------------------------------------------------------------------------
    /// Input gain in decibels (0 to +20 dB).
    ///
    /// Sets the initial signal level before the preamp stages.
    /// Higher values drive the preamp harder, increasing distortion.
    pub input_gain_db: f32,

    /// Pickup resonance frequency in Hz (2000-5000 Hz typical).
    ///
    /// Simulates the resonant peak of a guitar pickup.
    /// Single coils: ~3-4 kHz, Humbuckers: ~2-3 kHz.
    pub pickup_freq: f32,

    /// Pickup resonance Q factor (0.5-2.0).
    ///
    /// Controls the sharpness of the pickup resonance.
    /// Higher Q = more pronounced resonance peak.
    pub pickup_q: f32,

    // -------------------------------------------------------------------------
    // Preamp Stages
    // -------------------------------------------------------------------------
    /// Number of active preamp stages (1-4).
    ///
    /// More stages = more gain and harmonic complexity.
    /// Clean: 1-2, Crunch: 2-3, High gain: 3-4.
    pub num_stages: usize,

    /// Gain for each preamp stage (1.0-100.0).
    ///
    /// Array index corresponds to stage number.
    /// Unused stages (index >= num_stages) should be 0.0.
    pub stage_gains: [f32; 4],

    /// Asymmetry factor for each stage waveshaper (0.0-0.5).
    ///
    /// Controls even harmonic content from asymmetric clipping.
    /// 0.0 = symmetric, 0.5 = maximum asymmetry.
    pub stage_asymmetry: [f32; 4],

    /// Coupling capacitor cutoff frequency per stage in Hz (7-50 Hz).
    ///
    /// High-pass filter between stages that removes DC and affects
    /// low-frequency response. Lower values = more bass retention.
    pub coupling_fc: [f32; 4],

    /// Grid conduction threshold per stage (0.0-1.0).
    ///
    /// Point at which asymmetric soft clipping begins.
    /// Lower values = earlier onset of grid blocking distortion.
    pub grid_threshold: [f32; 4],

    // -------------------------------------------------------------------------
    // Tone Stack
    // -------------------------------------------------------------------------
    /// Tone stack EQ topology.
    ///
    /// Determines the frequency centers and interaction between controls.
    /// See [`ToneStackType`] for available topologies.
    pub tone_stack_type: ToneStackType,

    /// Bass control (0.0-1.0).
    ///
    /// 0.5 = neutral, below = cut, above = boost.
    pub bass: f32,

    /// Mid control (0.0-1.0).
    ///
    /// 0.5 = neutral, below = cut, above = boost.
    pub mid: f32,

    /// Treble control (0.0-1.0).
    ///
    /// 0.5 = neutral, below = cut, above = boost.
    /// Note: For Vox topology, higher values reduce treble (cut control).
    pub treble: f32,

    // -------------------------------------------------------------------------
    // Power Amp
    // -------------------------------------------------------------------------
    /// Push-pull crossover distortion amount (0.0-1.0).
    ///
    /// Simulates the dead zone between push and pull output tubes.
    /// Higher values = more crossover distortion at low volumes.
    pub crossover_amount: f32,

    /// Power supply sag depth (0.0-1.0).
    ///
    /// Amount of gain reduction under heavy signal load.
    /// Higher values = more "bloom" and compression feel.
    pub sag_depth: f32,

    /// Sag attack time in milliseconds (10-100 ms).
    ///
    /// How quickly the power supply responds to transients.
    /// Shorter = faster compression onset.
    pub sag_attack_ms: f32,

    /// Sag release time in milliseconds (50-500 ms).
    ///
    /// How quickly gain recovers after transients.
    /// Longer = more sustained compression feel.
    pub sag_release_ms: f32,

    /// Output transformer low-pass cutoff in Hz (4000-10000 Hz).
    ///
    /// Simulates the bandwidth limiting of the output transformer.
    /// Lower values = darker, more vintage character.
    pub transformer_fc: f32,

    // -------------------------------------------------------------------------
    // Cabinet
    // -------------------------------------------------------------------------
    /// Cabinet impulse response index (0-3).
    ///
    /// Selects which cabinet IR to use:
    /// - 0: 1x12 American (bright, focused)
    /// - 1: 2x12 British (warm, midrange)
    /// - 2: 4x12 Heavy (deep, full)
    /// - 3: 1x10 Vintage (thin, nasal)
    pub cabinet_ir_index: usize,

    // -------------------------------------------------------------------------
    // Output
    // -------------------------------------------------------------------------
    /// Master volume in decibels (0 to -60 dB).
    ///
    /// Final output level control after all processing.
    /// Use negative values to reduce overall volume.
    pub master_volume_db: f32,
}

// =============================================================================
// Preset Constants
// =============================================================================

/// Clean Twin preset - Fender Twin Reverb character.
///
/// Bright, pristine clean tones with exceptional headroom.
/// Two preamp stages provide smooth, warm cleans without breakup.
///
/// **Character:** Sparkling highs, scooped mids, deep lows
/// **Best for:** Jazz, country, clean rhythm, pedal platform
///
/// | Parameter | Value | Rationale |
/// |-----------|-------|-----------|
/// | Stages | 2 | Minimal distortion |
/// | Tone Stack | Fender | Classic American EQ |
/// | Sag | Low | Maximum headroom |
pub const CLEAN_TWIN: AmpPreset = AmpPreset {
    name: "clean_twin",

    // Input - moderate gain, bright pickup simulation
    input_gain_db: 6.0,
    pickup_freq: 3500.0,
    pickup_q: 1.2,

    // Preamp - 2 stages, low gain, minimal asymmetry
    num_stages: 2,
    stage_gains: [30.0, 25.0, 0.0, 0.0],
    stage_asymmetry: [0.05, 0.05, 0.0, 0.0],
    coupling_fc: [10.0, 10.0, 0.0, 0.0],
    grid_threshold: [0.85, 0.85, 0.0, 0.0],

    // Tone stack - Fender, bright settings
    tone_stack_type: ToneStackType::Fender,
    bass: 0.5,
    mid: 0.4,
    treble: 0.65,

    // Power amp - minimal sag, clean headroom
    crossover_amount: 0.02,
    sag_depth: 0.1,
    sag_attack_ms: 30.0,
    sag_release_ms: 200.0,
    transformer_fc: 8000.0,

    // Cabinet - 1x12 American for clarity
    cabinet_ir_index: 0,

    // Output - moderate level
    master_volume_db: -6.0,
};

/// Tweed Deluxe preset - Fender Deluxe character.
///
/// Warm, musical breakup with touch-sensitive dynamics.
/// The classic recording amp sound with natural compression.
///
/// **Character:** Warm, woody, responsive to pick attack
/// **Best for:** Blues, classic rock, roots, studio recording
///
/// | Parameter | Value | Rationale |
/// |-----------|-------|-----------|
/// | Stages | 2 | Edge of breakup |
/// | Tone Stack | Fender | Mid-scooped warmth |
/// | Sag | Medium | Musical compression |
pub const TWEED_DELUXE: AmpPreset = AmpPreset {
    name: "tweed_deluxe",

    // Input - higher gain to push into breakup
    input_gain_db: 10.0,
    pickup_freq: 3000.0,
    pickup_q: 1.0,

    // Preamp - 2 stages, higher gain, more asymmetry for warmth
    num_stages: 2,
    stage_gains: [45.0, 40.0, 0.0, 0.0],
    stage_asymmetry: [0.15, 0.2, 0.0, 0.0],
    coupling_fc: [12.0, 15.0, 0.0, 0.0],
    grid_threshold: [0.7, 0.65, 0.0, 0.0],

    // Tone stack - Fender, warmer settings
    tone_stack_type: ToneStackType::Fender,
    bass: 0.55,
    mid: 0.5,
    treble: 0.5,

    // Power amp - moderate sag for bloom
    crossover_amount: 0.05,
    sag_depth: 0.35,
    sag_attack_ms: 25.0,
    sag_release_ms: 180.0,
    transformer_fc: 6500.0,

    // Cabinet - 1x12 American
    cabinet_ir_index: 0,

    // Output
    master_volume_db: -6.0,
};

/// Plexi Crunch preset - Marshall JTM45 character.
///
/// Classic British rock tone with aggressive midrange.
/// Three stages provide singing sustain and harmonic richness.
///
/// **Character:** Crunchy, punchy mids, classic rock
/// **Best for:** Classic rock, blues rock, hard rock rhythm
///
/// | Parameter | Value | Rationale |
/// |-----------|-------|-----------|
/// | Stages | 3 | Medium gain structure |
/// | Tone Stack | Marshall | Mid-forward British EQ |
/// | Sag | Medium | Dynamic response |
pub const PLEXI_CRUNCH: AmpPreset = AmpPreset {
    name: "plexi_crunch",

    // Input - moderate gain
    input_gain_db: 8.0,
    pickup_freq: 2800.0,
    pickup_q: 0.9,

    // Preamp - 3 stages, progressive gain
    num_stages: 3,
    stage_gains: [35.0, 40.0, 35.0, 0.0],
    stage_asymmetry: [0.12, 0.18, 0.15, 0.0],
    coupling_fc: [15.0, 20.0, 18.0, 0.0],
    grid_threshold: [0.75, 0.68, 0.72, 0.0],

    // Tone stack - Marshall, mid-forward
    tone_stack_type: ToneStackType::Marshall,
    bass: 0.5,
    mid: 0.6,
    treble: 0.55,

    // Power amp - responsive sag
    crossover_amount: 0.06,
    sag_depth: 0.3,
    sag_attack_ms: 20.0,
    sag_release_ms: 150.0,
    transformer_fc: 6000.0,

    // Cabinet - 2x12 British for midrange
    cabinet_ir_index: 1,

    // Output
    master_volume_db: -6.0,
};

/// Brit High preset - Marshall JCM800 character.
///
/// High-gain British tone with aggressive attack and sustain.
/// The definitive hard rock and metal rhythm tone.
///
/// **Character:** Aggressive, tight, cutting
/// **Best for:** Hard rock, 80s metal, punk, heavy rhythm
///
/// | Parameter | Value | Rationale |
/// |-----------|-------|-----------|
/// | Stages | 3 | High gain saturation |
/// | Tone Stack | Marshall | Aggressive EQ |
/// | Sag | Lower | Tight response |
pub const BRIT_HIGH: AmpPreset = AmpPreset {
    name: "brit_high",

    // Input - higher gain for saturation
    input_gain_db: 12.0,
    pickup_freq: 2600.0,
    pickup_q: 0.85,

    // Preamp - 3 stages, high gain
    num_stages: 3,
    stage_gains: [50.0, 55.0, 45.0, 0.0],
    stage_asymmetry: [0.18, 0.22, 0.2, 0.0],
    coupling_fc: [20.0, 25.0, 22.0, 0.0],
    grid_threshold: [0.65, 0.58, 0.62, 0.0],

    // Tone stack - Marshall, bright and aggressive
    tone_stack_type: ToneStackType::Marshall,
    bass: 0.45,
    mid: 0.65,
    treble: 0.65,

    // Power amp - tighter sag for definition
    crossover_amount: 0.08,
    sag_depth: 0.25,
    sag_attack_ms: 15.0,
    sag_release_ms: 120.0,
    transformer_fc: 5500.0,

    // Cabinet - 4x12 Heavy for fullness
    cabinet_ir_index: 2,

    // Output
    master_volume_db: -6.0,
};

/// AC30 Chime preset - Vox AC30 character.
///
/// Chimey, jangly British tone with distinctive top-boost sound.
/// The signature sound of British invasion and indie rock.
///
/// **Character:** Chimey, bright, jangly, shimmering
/// **Best for:** Indie, jangle pop, British invasion, clean leads
///
/// | Parameter | Value | Rationale |
/// |-----------|-------|-----------|
/// | Stages | 3 | Medium gain, rich harmonics |
/// | Tone Stack | Vox | Unique cut control EQ |
/// | Sag | Medium | Bouncy dynamics |
pub const AC30_CHIME: AmpPreset = AmpPreset {
    name: "ac30_chime",

    // Input - moderate gain
    input_gain_db: 9.0,
    pickup_freq: 3200.0,
    pickup_q: 1.1,

    // Preamp - 3 stages, chimey character
    num_stages: 3,
    stage_gains: [38.0, 42.0, 36.0, 0.0],
    stage_asymmetry: [0.1, 0.15, 0.12, 0.0],
    coupling_fc: [12.0, 14.0, 12.0, 0.0],
    grid_threshold: [0.78, 0.72, 0.75, 0.0],

    // Tone stack - Vox, bright with moderate cut
    tone_stack_type: ToneStackType::Vox,
    bass: 0.5,
    mid: 0.55,
    treble: 0.35, // Lower = less cut = brighter

    // Power amp - bouncy sag
    crossover_amount: 0.04,
    sag_depth: 0.32,
    sag_attack_ms: 22.0,
    sag_release_ms: 160.0,
    transformer_fc: 7000.0,

    // Cabinet - 2x12 British for classic Vox pairing
    cabinet_ir_index: 1,

    // Output
    master_volume_db: -6.0,
};

/// Recto Heavy preset - Modern high-gain character.
///
/// Massive, saturated modern high-gain tone with scooped mids.
/// Four preamp stages deliver crushing distortion with tight low end.
///
/// **Character:** Heavy, saturated, tight, scooped
/// **Best for:** Modern metal, djent, deathcore, drop tunings
///
/// | Parameter | Value | Rationale |
/// |-----------|-------|-----------|
/// | Stages | 4 | Maximum saturation |
/// | Tone Stack | Fender | Scooped modern EQ |
/// | Sag | Low | Tight, precise response |
pub const RECTO_HEAVY: AmpPreset = AmpPreset {
    name: "recto_heavy",

    // Input - high gain for saturation
    input_gain_db: 15.0,
    pickup_freq: 2400.0,
    pickup_q: 0.8,

    // Preamp - 4 stages, high gain throughout
    num_stages: 4,
    stage_gains: [55.0, 60.0, 55.0, 50.0],
    stage_asymmetry: [0.2, 0.25, 0.22, 0.18],
    coupling_fc: [25.0, 30.0, 28.0, 25.0],
    grid_threshold: [0.6, 0.52, 0.55, 0.58],

    // Tone stack - Fender but scooped for modern metal
    tone_stack_type: ToneStackType::Fender,
    bass: 0.65,
    mid: 0.3, // Scooped mids
    treble: 0.6,

    // Power amp - tight response for precision
    crossover_amount: 0.1,
    sag_depth: 0.15,
    sag_attack_ms: 12.0,
    sag_release_ms: 100.0,
    transformer_fc: 5000.0,

    // Cabinet - 4x12 Heavy for maximum depth
    cabinet_ir_index: 2,

    // Output - slightly lower to compensate for high gain
    master_volume_db: -9.0,
};

// =============================================================================
// Preset Access Functions
// =============================================================================

/// Array of all available presets for iteration.
///
/// This static array holds references to all preset constants.
static ALL_PRESETS: [AmpPreset; 6] = [
    CLEAN_TWIN,
    TWEED_DELUXE,
    PLEXI_CRUNCH,
    BRIT_HIGH,
    AC30_CHIME,
    RECTO_HEAVY,
];

/// Returns a slice containing all available amp presets.
///
/// Use this to iterate over presets for UI display or preset cycling.
///
/// # Returns
///
/// A static slice of all [`AmpPreset`] instances.
///
/// # Example
///
/// ```ignore
/// for preset in all_presets() {
///     println!("{}: {} stages, {:?} stack",
///         preset.name,
///         preset.num_stages,
///         preset.tone_stack_type);
/// }
/// ```
#[must_use]
pub fn all_presets() -> &'static [AmpPreset] {
    &ALL_PRESETS
}

/// Finds a preset by its name.
///
/// Performs a case-sensitive string comparison against preset names.
///
/// # Arguments
///
/// * `name` - The preset name to search for (e.g., "clean_twin", "plexi_crunch")
///
/// # Returns
///
/// `Some(AmpPreset)` if found, `None` otherwise.
///
/// # Example
///
/// ```ignore
/// if let Some(preset) = preset_by_name("brit_high") {
///     // Configure signal chain with this preset
///     configure_amp(&preset);
/// } else {
///     // Handle unknown preset name
///     eprintln!("Unknown preset name");
/// }
/// ```
#[must_use]
pub fn preset_by_name(name: &str) -> Option<AmpPreset> {
    all_presets().iter().find(|p| p.name == name).copied()
}

/// Returns the number of available presets.
///
/// # Returns
///
/// The count of defined amp presets (currently 6).
#[must_use]
pub const fn preset_count() -> usize {
    6
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Preset Existence Tests (G1.1-G1.6)
    // -------------------------------------------------------------------------

    #[test]
    fn test_clean_twin_exists() {
        assert_eq!(CLEAN_TWIN.name, "clean_twin");
    }

    #[test]
    fn test_tweed_deluxe_exists() {
        assert_eq!(TWEED_DELUXE.name, "tweed_deluxe");
    }

    #[test]
    fn test_plexi_crunch_exists() {
        assert_eq!(PLEXI_CRUNCH.name, "plexi_crunch");
    }

    #[test]
    fn test_brit_high_exists() {
        assert_eq!(BRIT_HIGH.name, "brit_high");
    }

    #[test]
    fn test_ac30_chime_exists() {
        assert_eq!(AC30_CHIME.name, "ac30_chime");
    }

    #[test]
    fn test_recto_heavy_exists() {
        assert_eq!(RECTO_HEAVY.name, "recto_heavy");
    }

    // -------------------------------------------------------------------------
    // Stage Count Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_clean_twin_has_two_stages() {
        assert_eq!(CLEAN_TWIN.num_stages, 2);
    }

    #[test]
    fn test_tweed_deluxe_has_two_stages() {
        assert_eq!(TWEED_DELUXE.num_stages, 2);
    }

    #[test]
    fn test_plexi_crunch_has_three_stages() {
        assert_eq!(PLEXI_CRUNCH.num_stages, 3);
    }

    #[test]
    fn test_brit_high_has_three_stages() {
        assert_eq!(BRIT_HIGH.num_stages, 3);
    }

    #[test]
    fn test_ac30_chime_has_three_stages() {
        assert_eq!(AC30_CHIME.num_stages, 3);
    }

    #[test]
    fn test_recto_heavy_has_four_stages() {
        assert_eq!(RECTO_HEAVY.num_stages, 4);
    }

    // -------------------------------------------------------------------------
    // Tone Stack Type Tests (G3.2)
    // -------------------------------------------------------------------------

    #[test]
    fn test_clean_twin_fender_stack() {
        assert_eq!(CLEAN_TWIN.tone_stack_type, ToneStackType::Fender);
    }

    #[test]
    fn test_tweed_deluxe_fender_stack() {
        assert_eq!(TWEED_DELUXE.tone_stack_type, ToneStackType::Fender);
    }

    #[test]
    fn test_plexi_crunch_marshall_stack() {
        assert_eq!(PLEXI_CRUNCH.tone_stack_type, ToneStackType::Marshall);
    }

    #[test]
    fn test_brit_high_marshall_stack() {
        assert_eq!(BRIT_HIGH.tone_stack_type, ToneStackType::Marshall);
    }

    #[test]
    fn test_ac30_chime_vox_stack() {
        assert_eq!(AC30_CHIME.tone_stack_type, ToneStackType::Vox);
    }

    #[test]
    fn test_recto_heavy_fender_stack_scooped() {
        assert_eq!(RECTO_HEAVY.tone_stack_type, ToneStackType::Fender);
        // Verify scooped mids
        assert!(RECTO_HEAVY.mid < 0.5, "Recto should have scooped mids");
    }

    // -------------------------------------------------------------------------
    // All Presets Function Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_all_presets_returns_six() {
        assert_eq!(all_presets().len(), 6);
    }

    #[test]
    fn test_all_presets_contains_all_names() {
        let names: Vec<&str> = all_presets().iter().map(|p| p.name).collect();
        assert!(names.contains(&"clean_twin"));
        assert!(names.contains(&"tweed_deluxe"));
        assert!(names.contains(&"plexi_crunch"));
        assert!(names.contains(&"brit_high"));
        assert!(names.contains(&"ac30_chime"));
        assert!(names.contains(&"recto_heavy"));
    }

    #[test]
    fn test_preset_count() {
        assert_eq!(preset_count(), 6);
        assert_eq!(preset_count(), all_presets().len());
    }

    // -------------------------------------------------------------------------
    // Preset By Name Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_preset_by_name_finds_clean_twin() {
        let preset = preset_by_name("clean_twin");
        assert!(preset.is_some());
        assert_eq!(preset.unwrap().name, "clean_twin");
    }

    #[test]
    fn test_preset_by_name_finds_all_presets() {
        assert!(preset_by_name("clean_twin").is_some());
        assert!(preset_by_name("tweed_deluxe").is_some());
        assert!(preset_by_name("plexi_crunch").is_some());
        assert!(preset_by_name("brit_high").is_some());
        assert!(preset_by_name("ac30_chime").is_some());
        assert!(preset_by_name("recto_heavy").is_some());
    }

    #[test]
    fn test_preset_by_name_returns_none_for_unknown() {
        assert!(preset_by_name("unknown_preset").is_none());
        assert!(preset_by_name("").is_none());
        assert!(preset_by_name("Clean_Twin").is_none()); // Case sensitive
    }

    // -------------------------------------------------------------------------
    // Parameter Range Validation Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_all_presets_have_valid_num_stages() {
        for preset in all_presets() {
            assert!(
                preset.num_stages >= 1 && preset.num_stages <= 4,
                "{} has invalid num_stages: {}",
                preset.name,
                preset.num_stages
            );
        }
    }

    #[test]
    fn test_all_presets_have_valid_input_gain() {
        for preset in all_presets() {
            assert!(
                preset.input_gain_db >= 0.0 && preset.input_gain_db <= 20.0,
                "{} has invalid input_gain_db: {}",
                preset.name,
                preset.input_gain_db
            );
        }
    }

    #[test]
    fn test_all_presets_have_valid_tone_controls() {
        for preset in all_presets() {
            assert!(
                preset.bass >= 0.0 && preset.bass <= 1.0,
                "{} has invalid bass: {}",
                preset.name,
                preset.bass
            );
            assert!(
                preset.mid >= 0.0 && preset.mid <= 1.0,
                "{} has invalid mid: {}",
                preset.name,
                preset.mid
            );
            assert!(
                preset.treble >= 0.0 && preset.treble <= 1.0,
                "{} has invalid treble: {}",
                preset.name,
                preset.treble
            );
        }
    }

    #[test]
    fn test_all_presets_have_valid_sag_depth() {
        for preset in all_presets() {
            assert!(
                preset.sag_depth >= 0.0 && preset.sag_depth <= 1.0,
                "{} has invalid sag_depth: {}",
                preset.name,
                preset.sag_depth
            );
        }
    }

    #[test]
    fn test_all_presets_have_valid_cabinet_index() {
        for preset in all_presets() {
            assert!(
                preset.cabinet_ir_index <= 3,
                "{} has invalid cabinet_ir_index: {}",
                preset.name,
                preset.cabinet_ir_index
            );
        }
    }

    #[test]
    fn test_all_presets_have_valid_master_volume() {
        for preset in all_presets() {
            assert!(
                preset.master_volume_db <= 0.0 && preset.master_volume_db >= -60.0,
                "{} has invalid master_volume_db: {}",
                preset.name,
                preset.master_volume_db
            );
        }
    }

    // -------------------------------------------------------------------------
    // Character Validation Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_clean_twin_has_low_gain_character() {
        // Clean amp should have lower gain stages
        let max_stage_gain = CLEAN_TWIN
            .stage_gains
            .iter()
            .take(CLEAN_TWIN.num_stages)
            .fold(0.0_f32, |a, &b| a.max(b));
        assert!(
            max_stage_gain < 40.0,
            "Clean twin should have low stage gains"
        );
    }

    #[test]
    fn test_recto_heavy_has_high_gain_character() {
        // High gain amp should have higher gain stages
        let max_stage_gain = RECTO_HEAVY
            .stage_gains
            .iter()
            .take(RECTO_HEAVY.num_stages)
            .fold(0.0_f32, |a, &b| a.max(b));
        assert!(
            max_stage_gain > 50.0,
            "Recto heavy should have high stage gains"
        );
    }

    #[test]
    fn test_gain_progression_clean_to_heavy() {
        // Presets should have progressively more gain
        let clean_max = CLEAN_TWIN
            .stage_gains
            .iter()
            .take(CLEAN_TWIN.num_stages)
            .fold(0.0_f32, |a, &b| a.max(b));
        let heavy_max = RECTO_HEAVY
            .stage_gains
            .iter()
            .take(RECTO_HEAVY.num_stages)
            .fold(0.0_f32, |a, &b| a.max(b));

        assert!(
            heavy_max > clean_max,
            "Recto should have higher gain than Clean Twin"
        );
    }

    // -------------------------------------------------------------------------
    // Copy/Clone Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_preset_is_copy() {
        let preset = CLEAN_TWIN;
        let copy = preset;
        assert_eq!(preset.name, copy.name);
    }

    #[test]
    fn test_preset_clone() {
        let preset = PLEXI_CRUNCH;
        let cloned = preset.clone();
        assert_eq!(preset.name, cloned.name);
        assert_eq!(preset.num_stages, cloned.num_stages);
    }

    // -------------------------------------------------------------------------
    // Unique Names Test
    // -------------------------------------------------------------------------

    #[test]
    fn test_all_preset_names_unique() {
        let presets = all_presets();
        for i in 0..presets.len() {
            for j in (i + 1)..presets.len() {
                assert_ne!(
                    presets[i].name, presets[j].name,
                    "Duplicate preset name: {}",
                    presets[i].name
                );
            }
        }
    }
}
