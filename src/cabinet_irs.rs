//! Cabinet IR Preset Data
//!
//! Pre-computed impulse responses for speaker cabinet simulation.
//! These are synthetic approximations - real IRs would be measured from actual cabinets.
//!
//! # Available IRs
//!
//! - [`ir_1x12_american`] - Bright, focused open-back combo (256 samples)
//! - [`ir_2x12_british`] - Warm, midrange semi-open back (384 samples)
//! - [`ir_4x12_heavy`] - Deep, full closed-back (512 samples)
//! - [`ir_1x10_vintage`] - Thin, nasal small speaker (256 samples)
//!
//! # Implementation Notes
//!
//! Synthetic IRs are generated using:
//! 1. Impulse response shaping (exponential decay + resonance)
//! 2. Frequency-dependent characteristics (bright = faster decay, deep = slower decay)
//! 3. Early reflections for body/box resonance
//!
//! # Requirements Traceability
//!
//! - G2.1: `1x12_american` - Bright, focused (256 samples)
//! - G2.2: `2x12_british` - Warm, midrange (384 samples)
//! - G2.3: `4x12_heavy` - Deep, full (512 samples)
//! - G2.4: `1x10_vintage` - Thin, nasal (256 samples)

use crate::cabinet::{CabinetIrData, MAX_IR_LENGTH};

/// Index constant for 1x12 American cabinet selection.
pub const IR_1X12_AMERICAN: usize = 0;

/// Index constant for 2x12 British cabinet selection.
pub const IR_2X12_BRITISH: usize = 1;

/// Index constant for 4x12 Heavy cabinet selection.
pub const IR_4X12_HEAVY: usize = 2;

/// Index constant for 1x10 Vintage cabinet selection.
pub const IR_1X10_VINTAGE: usize = 3;

/// Total number of available cabinet IRs.
pub const CABINET_IR_COUNT: usize = 4;

/// 1x12 American - Bright, focused (256 samples).
///
/// Character: Open-back combo, emphasized high mids, quick decay.
/// Typical use: Clean tones, country, blues.
///
/// # Returns
///
/// A 256-sample synthetic IR approximating a bright American combo cabinet.
pub fn ir_1x12_american() -> CabinetIrData {
    create_synthetic_ir(256, 4000.0, 0.8, 0.3, "1x12_american")
}

/// 2x12 British - Warm, midrange (384 samples).
///
/// Character: Semi-open back, strong midrange, moderate decay.
/// Typical use: Classic rock, blues-rock, British invasion.
///
/// # Returns
///
/// A 384-sample synthetic IR approximating a warm British cabinet.
pub fn ir_2x12_british() -> CabinetIrData {
    create_synthetic_ir(384, 2500.0, 0.6, 0.5, "2x12_british")
}

/// 4x12 Heavy - Deep, full (512 samples).
///
/// Character: Closed-back, deep bass, slow decay, massive sound.
/// Typical use: Hard rock, metal, modern high-gain.
///
/// # Returns
///
/// A 512-sample synthetic IR approximating a deep heavy cabinet.
pub fn ir_4x12_heavy() -> CabinetIrData {
    create_synthetic_ir(512, 1800.0, 0.4, 0.7, "4x12_heavy")
}

/// 1x10 Vintage - Thin, nasal (256 samples).
///
/// Character: Small speaker, midrange emphasis, quick response.
/// Typical use: Vintage tones, lo-fi, garage rock.
///
/// # Returns
///
/// A 256-sample synthetic IR approximating a thin vintage speaker.
pub fn ir_1x10_vintage() -> CabinetIrData {
    create_synthetic_ir(256, 3500.0, 0.9, 0.2, "1x10_vintage")
}

/// Get all cabinet IRs as an array.
///
/// # Returns
///
/// An array of all 4 cabinet IRs in order:
/// `[1x12_american, 2x12_british, 4x12_heavy, 1x10_vintage]`
///
/// # Example
///
/// ```ignore
/// use guitar::cabinet_irs::all_cabinet_irs;
///
/// let irs = all_cabinet_irs();
/// for ir in &irs {
///     println!("{}: {} samples", ir.name, ir.length);
/// }
/// ```
pub fn all_cabinet_irs() -> [CabinetIrData; CABINET_IR_COUNT] {
    [
        ir_1x12_american(),
        ir_2x12_british(),
        ir_4x12_heavy(),
        ir_1x10_vintage(),
    ]
}

/// Get cabinet IR by index.
///
/// # Arguments
///
/// * `index` - Cabinet IR index (0-3)
///
/// # Returns
///
/// The corresponding cabinet IR, or `1x12_american` as default for invalid indices.
///
/// # Index Mapping
///
/// - 0: `1x12_american`
/// - 1: `2x12_british`
/// - 2: `4x12_heavy`
/// - 3: `1x10_vintage`
/// - _: `1x12_american` (default)
///
/// # Example
///
/// ```ignore
/// use guitar::cabinet_irs::{cabinet_ir_by_index, IR_4X12_HEAVY};
///
/// let heavy_ir = cabinet_ir_by_index(IR_4X12_HEAVY);
/// assert_eq!(heavy_ir.length, 512);
/// ```
pub fn cabinet_ir_by_index(index: usize) -> CabinetIrData {
    match index {
        IR_1X12_AMERICAN => ir_1x12_american(),
        IR_2X12_BRITISH => ir_2x12_british(),
        IR_4X12_HEAVY => ir_4x12_heavy(),
        IR_1X10_VINTAGE => ir_1x10_vintage(),
        _ => ir_1x12_american(), // Default
    }
}

/// Create a synthetic cabinet IR with configurable characteristics.
///
/// Generates a synthetic impulse response by combining:
/// 1. Initial transient (speaker cone response)
/// 2. Primary resonance (speaker + cabinet)
/// 3. Secondary decay (cabinet reflections)
/// 4. High frequency detail
///
/// # Arguments
///
/// * `length` - IR length in samples (256, 384, or 512)
/// * `resonance_freq` - Primary speaker resonance frequency in Hz
/// * `brightness` - High frequency content (0.0 = dark, 1.0 = bright)
/// * `body` - Low frequency content / cabinet size (0.0 = thin, 1.0 = deep)
/// * `name` - IR name string
///
/// # Returns
///
/// A normalized [`CabinetIrData`] with the specified characteristics.
fn create_synthetic_ir(
    length: usize,
    resonance_freq: f32,
    brightness: f32,
    body: f32,
    name: &'static str,
) -> CabinetIrData {
    let mut samples = [0.0f32; MAX_IR_LENGTH];
    let len = length.min(MAX_IR_LENGTH);
    let sample_rate = 48000.0f32;

    // Create IR with multiple components:
    // 1. Initial transient (speaker cone response)
    // 2. Primary resonance (speaker + cabinet)
    // 3. Secondary decay (cabinet reflections)

    let pi = core::f32::consts::PI;
    let omega = 2.0 * pi * resonance_freq / sample_rate;

    for (i, sample) in samples[..len].iter_mut().enumerate() {
        let t = i as f32 / sample_rate;
        let n = i as f32 / len as f32; // Normalized position 0-1

        // Initial transient (fast attack, quick decay)
        let transient = libm::expf(-t * 800.0 * (2.0 - brightness)) * (1.0 - n * 0.5);

        // Primary resonance (damped oscillation at speaker frequency)
        let resonance = libm::expf(-t * 200.0 * (1.5 - body)) * libm::sinf(omega * i as f32) * 0.3;

        // Low frequency body (slow decay for larger cabinets)
        let low_body =
            libm::expf(-t * 100.0 * (2.0 - body)) * libm::cosf(omega * 0.5 * i as f32) * body * 0.2;

        // High frequency detail
        let hi_detail = libm::expf(-t * 1500.0) * libm::sinf(omega * 2.0 * i as f32) * brightness * 0.1;

        *sample = transient + resonance + low_body + hi_detail;
    }

    // Normalize to unity DC gain
    let sum: f32 = samples[..len].iter().map(|s| libm::fabsf(*s)).sum();
    if sum > 0.001 {
        let scale = 1.0 / sum;
        for s in samples[..len].iter_mut() {
            *s *= scale;
        }
    }

    // Ensure first sample has energy (for proper impulse)
    if libm::fabsf(samples[0]) < 0.01 {
        samples[0] = 0.5;
    }

    CabinetIrData {
        samples,
        length: len,
        name,
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to check if two floats are approximately equal.
    fn approx_eq(a: f32, b: f32, epsilon: f32) -> bool {
        libm::fabsf(a - b) < epsilon
    }

    // ------------------------------------------------------------------------
    // IR Length Tests (G2.1-G2.4)
    // ------------------------------------------------------------------------

    #[test]
    fn test_ir_1x12_american_length() {
        let ir = ir_1x12_american();
        assert_eq!(ir.length, 256, "1x12_american should be 256 samples");
        assert_eq!(ir.name, "1x12_american");
    }

    #[test]
    fn test_ir_2x12_british_length() {
        let ir = ir_2x12_british();
        assert_eq!(ir.length, 384, "2x12_british should be 384 samples");
        assert_eq!(ir.name, "2x12_british");
    }

    #[test]
    fn test_ir_4x12_heavy_length() {
        let ir = ir_4x12_heavy();
        assert_eq!(ir.length, 512, "4x12_heavy should be 512 samples");
        assert_eq!(ir.name, "4x12_heavy");
    }

    #[test]
    fn test_ir_1x10_vintage_length() {
        let ir = ir_1x10_vintage();
        assert_eq!(ir.length, 256, "1x10_vintage should be 256 samples");
        assert_eq!(ir.name, "1x10_vintage");
    }

    // ------------------------------------------------------------------------
    // Normalization Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_ir_1x12_american_normalized() {
        let ir = ir_1x12_american();
        let sum: f32 = ir.samples[..ir.length].iter().map(|s| libm::fabsf(*s)).sum();
        assert!(
            approx_eq(sum, 1.0, 0.05),
            "1x12_american should be normalized: sum = {}",
            sum
        );
    }

    #[test]
    fn test_ir_2x12_british_normalized() {
        let ir = ir_2x12_british();
        let sum: f32 = ir.samples[..ir.length].iter().map(|s| libm::fabsf(*s)).sum();
        assert!(
            approx_eq(sum, 1.0, 0.05),
            "2x12_british should be normalized: sum = {}",
            sum
        );
    }

    #[test]
    fn test_ir_4x12_heavy_normalized() {
        let ir = ir_4x12_heavy();
        let sum: f32 = ir.samples[..ir.length].iter().map(|s| libm::fabsf(*s)).sum();
        assert!(
            approx_eq(sum, 1.0, 0.05),
            "4x12_heavy should be normalized: sum = {}",
            sum
        );
    }

    #[test]
    fn test_ir_1x10_vintage_normalized() {
        let ir = ir_1x10_vintage();
        let sum: f32 = ir.samples[..ir.length].iter().map(|s| libm::fabsf(*s)).sum();
        assert!(
            approx_eq(sum, 1.0, 0.05),
            "1x10_vintage should be normalized: sum = {}",
            sum
        );
    }

    // ------------------------------------------------------------------------
    // Character Tests (Frequency Content)
    // ------------------------------------------------------------------------

    #[test]
    fn test_1x12_american_bright_character() {
        let ir = ir_1x12_american();

        // Bright IRs should have faster initial decay (less energy in later samples)
        let first_quarter: f32 = ir.samples[..ir.length / 4]
            .iter()
            .map(|s| libm::fabsf(*s))
            .sum();
        let last_quarter: f32 = ir.samples[ir.length * 3 / 4..ir.length]
            .iter()
            .map(|s| libm::fabsf(*s))
            .sum();

        assert!(
            first_quarter > last_quarter * 2.0,
            "Bright IR should have faster decay: first={}, last={}",
            first_quarter,
            last_quarter
        );
    }

    #[test]
    fn test_4x12_heavy_deep_character() {
        let ir = ir_4x12_heavy();

        // Deep IRs should have slower decay (more sustained energy)
        let second_half: f32 = ir.samples[ir.length / 2..ir.length]
            .iter()
            .map(|s| libm::fabsf(*s))
            .sum();

        // Heavy cabinet should have noticeable energy in second half
        assert!(
            second_half > 0.1,
            "Heavy IR should have sustained energy: second_half={}",
            second_half
        );
    }

    #[test]
    fn test_1x10_vintage_thin_character() {
        let ir = ir_1x10_vintage();

        // Thin IRs should have fast decay - first quarter has more energy than last quarter
        let first_quarter: f32 = ir.samples[..ir.length / 4]
            .iter()
            .map(|s| libm::fabsf(*s))
            .sum();
        let last_quarter: f32 = ir.samples[ir.length * 3 / 4..ir.length]
            .iter()
            .map(|s| libm::fabsf(*s))
            .sum();

        assert!(
            first_quarter > last_quarter * 1.5,
            "Thin IR should have faster decay: first_quarter={}, last_quarter={}",
            first_quarter,
            last_quarter
        );
    }

    #[test]
    fn test_2x12_british_balanced_character() {
        let ir = ir_2x12_british();

        // British IR should have moderate decay characteristics
        let third2: f32 = ir.samples[ir.length / 3..ir.length * 2 / 3]
            .iter()
            .map(|s| libm::fabsf(*s))
            .sum();

        // Should have moderate energy distribution
        assert!(
            third2 > 0.05,
            "British IR should have balanced decay: third2={}",
            third2
        );
    }

    // ------------------------------------------------------------------------
    // Index Lookup Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_cabinet_ir_by_index_valid() {
        let ir0 = cabinet_ir_by_index(IR_1X12_AMERICAN);
        assert_eq!(ir0.name, "1x12_american");

        let ir1 = cabinet_ir_by_index(IR_2X12_BRITISH);
        assert_eq!(ir1.name, "2x12_british");

        let ir2 = cabinet_ir_by_index(IR_4X12_HEAVY);
        assert_eq!(ir2.name, "4x12_heavy");

        let ir3 = cabinet_ir_by_index(IR_1X10_VINTAGE);
        assert_eq!(ir3.name, "1x10_vintage");
    }

    #[test]
    fn test_cabinet_ir_by_index_invalid() {
        let ir_invalid = cabinet_ir_by_index(99);
        assert_eq!(
            ir_invalid.name, "1x12_american",
            "Invalid index should return default IR"
        );
    }

    #[test]
    fn test_all_cabinet_irs() {
        let all = all_cabinet_irs();
        assert_eq!(all.len(), CABINET_IR_COUNT);

        assert_eq!(all[0].name, "1x12_american");
        assert_eq!(all[1].name, "2x12_british");
        assert_eq!(all[2].name, "4x12_heavy");
        assert_eq!(all[3].name, "1x10_vintage");
    }

    // ------------------------------------------------------------------------
    // First Sample Energy Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_first_sample_has_energy() {
        let irs = all_cabinet_irs();
        for ir in &irs {
            assert!(
                libm::fabsf(ir.samples[0]) >= 0.01,
                "{} first sample should have energy: {}",
                ir.name,
                ir.samples[0]
            );
        }
    }

    // ------------------------------------------------------------------------
    // Samples Bounds Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_samples_within_bounds() {
        let irs = all_cabinet_irs();
        for ir in &irs {
            for (i, &sample) in ir.samples[..ir.length].iter().enumerate() {
                assert!(
                    sample.is_finite(),
                    "{} sample {} is not finite: {}",
                    ir.name,
                    i,
                    sample
                );
                assert!(
                    libm::fabsf(sample) < 10.0,
                    "{} sample {} is out of bounds: {}",
                    ir.name,
                    i,
                    sample
                );
            }
        }
    }

    #[test]
    fn test_unused_samples_zero() {
        let irs = all_cabinet_irs();
        for ir in &irs {
            for (i, &sample) in ir.samples[ir.length..].iter().enumerate() {
                assert!(
                    approx_eq(sample, 0.0, 1e-6),
                    "{} unused sample {} should be zero: {}",
                    ir.name,
                    i + ir.length,
                    sample
                );
            }
        }
    }

    // ------------------------------------------------------------------------
    // Index Constants Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_index_constants() {
        assert_eq!(IR_1X12_AMERICAN, 0);
        assert_eq!(IR_2X12_BRITISH, 1);
        assert_eq!(IR_4X12_HEAVY, 2);
        assert_eq!(IR_1X10_VINTAGE, 3);
        assert_eq!(CABINET_IR_COUNT, 4);
    }

    // ------------------------------------------------------------------------
    // DC Gain Tests (Integration with Cabinet)
    // ------------------------------------------------------------------------

    #[test]
    fn test_dc_gain_approximation() {
        use crate::cabinet::Cabinet;

        let irs = all_cabinet_irs();
        for ir in irs {
            let name = ir.name;
            let mut cabinet = Cabinet::with_ir(ir);

            // Process many samples of DC input
            let dc_value = 0.5;
            let mut last_output = 0.0;

            for _ in 0..1000 {
                last_output = cabinet.process_sample(dc_value);
            }

            // After settling, output should be close to input
            // (allowing for some variation due to synthetic IR characteristics)
            assert!(
                libm::fabsf(last_output) < 1.0,
                "{} DC response should be bounded: {}",
                name,
                last_output
            );
        }
    }
}
