//! Power Amp Module
//!
//! Models power tube compression, push-pull crossover distortion,
//! power supply sag, and output transformer characteristics.
//!
//! # Overview
//!
//! This module implements three key power amplifier behaviors:
//!
//! 1. **Push-pull crossover distortion**: Models the dead zone where
//!    push-pull output tubes transition, adding subtle harmonic content
//!    at low signal levels.
//!
//! 2. **Power supply sag**: Simulates the drooping power supply voltage
//!    under heavy load, creating the characteristic "bloom" and compression
//!    of tube power amps.
//!
//! 3. **Output transformer low-pass**: Models the bandwidth limitation
//!    of the output transformer, rolling off harsh high frequencies.
//!
//! # Usage in Guitar Amp DSP
//!
//! The power amp stage comes after the tone stack (E4) and before
//! the cabinet simulation (E6) in the signal chain:
//!
//! ```text
//! Input -> Preamp -> Tone Stack -> Power Amp -> Cabinet -> Output
//! ```
//!
//! # Design Philosophy
//!
//! - **Immutable parameters**: Configuration via `PowerAmpParams` struct
//! - **Mutable state**: Only envelope and filter state change during processing
//! - **No heap allocation**: Fixed-size struct suitable for `no_std`
//! - **Builder pattern**: Use `with_params` for construction with custom parameters
//!
//! # References
//!
//! - tube_amp_emulation_spec.md section 3.5
//! - design.md requirements E5.1 through E5.4

use crate::biquad::Biquad;
use crate::dsp_math::one_pole_coeff;

/// Power amp parameters.
///
/// All parameters have sensible defaults suitable for general use.
/// Values are clamped to valid ranges during construction.
///
/// # Parameter Ranges
///
/// | Parameter | Min | Max | Default | Unit |
/// |-----------|-----|-----|---------|------|
/// | `crossover_amount` | 0.0 | 0.1 | 0.02 | - |
/// | `sag_depth` | 0.0 | 1.0 | 0.3 | - |
/// | `sag_attack_ms` | 10.0 | 100.0 | 30.0 | ms |
/// | `sag_release_ms` | 50.0 | 500.0 | 200.0 | ms |
/// | `transformer_fc` | 4000.0 | 10000.0 | 6000.0 | Hz |
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerAmpParams {
    /// Crossover dead zone width (0.0 to 0.1).
    ///
    /// Models the push-pull transition region where both output tubes
    /// are partially off. Higher values create more crossover distortion.
    /// Set to 0.0 to disable crossover modeling.
    pub crossover_amount: f32,

    /// Sag depth (0.0 to 1.0) - how much the power supply droops.
    ///
    /// Higher values create more compression and "bloom" on sustained notes.
    /// 0.0 disables sag entirely.
    pub sag_depth: f32,

    /// Sag attack time in milliseconds (10-100).
    ///
    /// How quickly the power supply starts to droop under load.
    /// Lower values create faster attack compression.
    pub sag_attack_ms: f32,

    /// Sag release time in milliseconds (50-500).
    ///
    /// How quickly the power supply recovers after the signal drops.
    /// Higher values create longer sustain/bloom effect.
    pub sag_release_ms: f32,

    /// Output transformer cutoff frequency in Hz (4000-10000).
    ///
    /// Models the bandwidth limitation of the output transformer.
    /// Lower values create a darker, warmer tone.
    pub transformer_fc: f32,
}

impl Default for PowerAmpParams {
    fn default() -> Self {
        Self {
            crossover_amount: 0.02,
            sag_depth: 0.3,
            sag_attack_ms: 30.0,
            sag_release_ms: 200.0,
            transformer_fc: 6000.0,
        }
    }
}

/// Power amp processor.
///
/// Combines push-pull crossover distortion, power supply sag compression,
/// and output transformer low-pass filtering into a single processing stage.
///
/// # Example
///
/// ```ignore
/// use crate::poweramp::{PowerAmp, PowerAmpParams};
///
/// // Create with default parameters
/// let mut amp = PowerAmp::new(48000.0);
///
/// // Process audio
/// let output = amp.process_sample(input);
///
/// // Or process a buffer
/// amp.process_buffer(&mut audio_buffer);
/// ```
#[derive(Debug, Clone)]
pub struct PowerAmp {
    /// Envelope follower state for sag calculation
    envelope: f32,
    /// Attack coefficient for envelope (samples to reach 63%)
    attack_coeff: f32,
    /// Release coefficient for envelope (samples to reach 63%)
    release_coeff: f32,
    /// Output transformer low-pass filter
    transformer: Biquad,
    /// Current parameters
    params: PowerAmpParams,
    /// Sample rate for coefficient recalculation
    sample_rate: f32,
}

impl PowerAmp {
    /// Creates a new power amp processor with default parameters.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Audio sample rate in Hz (e.g., 48000.0)
    ///
    /// # Returns
    ///
    /// A `PowerAmp` instance configured with default parameters.
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let params = PowerAmpParams::default();
        Self::with_params(params, sample_rate)
    }

    /// Creates a new power amp processor with custom parameters.
    ///
    /// Parameters are clamped to valid ranges automatically.
    ///
    /// # Arguments
    ///
    /// * `params` - Power amp configuration parameters
    /// * `sample_rate` - Audio sample rate in Hz
    ///
    /// # Returns
    ///
    /// A `PowerAmp` instance configured with the specified parameters.
    #[must_use]
    pub fn with_params(params: PowerAmpParams, sample_rate: f32) -> Self {
        let attack_ms = params.sag_attack_ms.clamp(10.0, 100.0);
        let release_ms = params.sag_release_ms.clamp(50.0, 500.0);

        // Convert time constants to filter coefficients
        // one_pole_coeff expects cutoff frequency, so we convert from time constant
        // Time constant tau = 1 / (2 * pi * fc), so fc = 1 / (2 * pi * tau)
        // For ms to seconds: fc = 1000 / (2 * pi * tau_ms)
        let attack_fc = 1000.0 / (2.0 * core::f32::consts::PI * attack_ms);
        let release_fc = 1000.0 / (2.0 * core::f32::consts::PI * release_ms);

        Self {
            envelope: 0.0,
            attack_coeff: 1.0 - one_pole_coeff(attack_fc, sample_rate),
            release_coeff: 1.0 - one_pole_coeff(release_fc, sample_rate),
            transformer: Biquad::low_pass(
                params.transformer_fc.clamp(4000.0, 10000.0),
                0.707,
                sample_rate,
            ),
            params,
            sample_rate,
        }
    }

    /// Updates the power amp parameters.
    ///
    /// This recalculates all internal coefficients based on the new parameters.
    /// Call `reset()` after changing parameters if you want to clear the
    /// envelope state.
    ///
    /// # Arguments
    ///
    /// * `params` - New power amp configuration parameters
    pub fn set_params(&mut self, params: PowerAmpParams) {
        *self = Self::with_params(params, self.sample_rate);
    }

    /// Returns the current parameters.
    ///
    /// # Returns
    ///
    /// A copy of the current `PowerAmpParams`.
    #[must_use]
    pub fn params(&self) -> PowerAmpParams {
        self.params
    }

    /// Processes a single audio sample through the power amp.
    ///
    /// The processing chain is:
    /// 1. Push-pull crossover distortion
    /// 2. Power supply sag compression
    /// 3. Output transformer low-pass filter
    ///
    /// # Arguments
    ///
    /// * `x` - Input audio sample
    ///
    /// # Returns
    ///
    /// Processed output sample.
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        // 1. Push-pull crossover distortion
        let crossed = crossover_model(x, self.params.crossover_amount);

        // 2. Power supply sag
        let sagged = self.apply_sag(crossed);

        // 3. Output transformer low-pass
        self.transformer.process_sample(sagged)
    }

    /// Applies power supply sag to the signal.
    ///
    /// Uses an envelope follower to track signal level, then reduces
    /// gain proportionally to simulate power supply droop.
    ///
    /// # Arguments
    ///
    /// * `x` - Input sample
    ///
    /// # Returns
    ///
    /// Sample with sag applied.
    #[inline]
    fn apply_sag(&mut self, x: f32) -> f32 {
        let rect = libm::fabsf(x);

        // Envelope follower with asymmetric attack/release
        if rect > self.envelope {
            self.envelope += self.attack_coeff * (rect - self.envelope);
        } else {
            self.envelope += self.release_coeff * (rect - self.envelope);
        }

        // Calculate sag amount: reduce gain based on envelope level
        // At high envelope levels, gain is reduced by up to sag_depth
        let sag_reduction = (self.envelope * self.params.sag_depth).min(self.params.sag_depth);
        let sag_amount = 1.0 - sag_reduction;

        x * sag_amount
    }

    /// Processes a buffer of audio samples in-place.
    ///
    /// More efficient than calling `process_sample` in a loop due to
    /// better cache locality.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Mutable slice of audio samples to process in-place.
    #[inline]
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Resets the power amp state.
    ///
    /// Clears the envelope follower and transformer filter state.
    /// Call this when switching presets or processing a new audio stream.
    pub fn reset(&mut self) {
        self.envelope = 0.0;
        self.transformer.reset();
    }

    /// Returns the current envelope level (for metering/debugging).
    ///
    /// # Returns
    ///
    /// The current envelope follower value (0.0 to ~1.0).
    #[must_use]
    pub fn envelope_level(&self) -> f32 {
        self.envelope
    }
}

/// Push-pull crossover distortion model.
///
/// Models the dead zone in push-pull output stages where both tubes
/// are partially off. This adds subtle harmonic content at low signal levels.
///
/// # Arguments
///
/// * `x` - Input sample
/// * `dead_zone` - Size of the crossover dead zone (0.0 to 0.1 typical)
///
/// # Returns
///
/// Sample with crossover distortion applied.
///
/// # Algorithm
///
/// For signals within the dead zone (|x| < dead_zone), the output is
/// attenuated by factor (|x| / dead_zone), creating a smooth transition
/// through zero rather than a hard kink.
#[inline]
fn crossover_model(x: f32, dead_zone: f32) -> f32 {
    // Bypass if crossover is disabled
    if dead_zone < 0.001 {
        return x;
    }

    let abs_x = libm::fabsf(x);
    if abs_x < dead_zone {
        // Smooth through dead zone: attenuate small signals
        x * (abs_x / dead_zone)
    } else {
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;
    const SAMPLE_RATE: f32 = 48000.0;

    fn approx_eq(a: f32, b: f32, epsilon: f32) -> bool {
        (a - b).abs() < epsilon
    }

    // ========================================================================
    // Crossover distortion tests
    // ========================================================================

    #[test]
    fn test_crossover_adds_distortion_at_low_levels() {
        let dead_zone = 0.05;
        let input = 0.02; // Within dead zone

        let output = crossover_model(input, dead_zone);

        // Output should be attenuated: output = input * (|input| / dead_zone)
        // output = 0.02 * (0.02 / 0.05) = 0.02 * 0.4 = 0.008
        let expected = input * (input / dead_zone);
        assert!(
            approx_eq(output, expected, EPSILON),
            "Crossover output {} expected {}",
            output,
            expected
        );
        assert!(
            output < input,
            "Crossover should attenuate within dead zone"
        );
    }

    #[test]
    fn test_crossover_bypasses_at_high_levels() {
        let dead_zone = 0.05;
        let input = 0.5; // Well above dead zone

        let output = crossover_model(input, dead_zone);

        assert!(
            approx_eq(output, input, EPSILON),
            "Crossover should pass through above dead zone: {} vs {}",
            output,
            input
        );
    }

    #[test]
    fn test_crossover_negative_signal() {
        let dead_zone = 0.05;
        let input = -0.02; // Negative, within dead zone

        let output = crossover_model(input, dead_zone);

        // Should be negative and attenuated
        assert!(output < 0.0, "Output should be negative");
        assert!(
            output > input,
            "Magnitude should be reduced: {} vs {}",
            output,
            input
        );
    }

    #[test]
    fn test_crossover_at_boundary() {
        let dead_zone = 0.05;
        let input = dead_zone; // Exactly at boundary

        let output = crossover_model(input, dead_zone);

        // At boundary: output = dead_zone * (dead_zone / dead_zone) = dead_zone
        assert!(
            approx_eq(output, input, EPSILON),
            "At boundary: {} vs {}",
            output,
            input
        );
    }

    #[test]
    fn test_crossover_bypass_when_zero_dead_zone() {
        let dead_zone = 0.0;
        let input = 0.01;

        let output = crossover_model(input, dead_zone);

        assert!(
            approx_eq(output, input, EPSILON),
            "Zero dead zone should bypass"
        );
    }

    #[test]
    fn test_crossover_zero_input() {
        let dead_zone = 0.05;
        let input = 0.0;

        let output = crossover_model(input, dead_zone);

        assert!(approx_eq(output, 0.0, EPSILON), "Zero in should be zero out");
    }

    // ========================================================================
    // Sag compression tests
    // ========================================================================

    #[test]
    fn test_sag_compresses_sustained_loud_signals() {
        let params = PowerAmpParams {
            crossover_amount: 0.0, // Disable crossover for isolated sag test
            sag_depth: 0.5,
            sag_attack_ms: 10.0,
            sag_release_ms: 100.0,
            transformer_fc: 20000.0, // High cutoff to minimize transformer effect
        };
        let mut amp = PowerAmp::with_params(params, SAMPLE_RATE);

        // Feed a sustained loud signal
        let input_level = 0.8;
        let mut last_output = 0.0;

        // Process many samples to let envelope rise
        for _ in 0..4800 {
            // 100ms at 48kHz
            last_output = amp.process_sample(input_level);
        }

        // Output should be lower than input due to sag
        assert!(
            last_output < input_level * 0.9,
            "Sag should reduce level: {} vs {}",
            last_output,
            input_level
        );

        // Envelope should have risen
        assert!(
            amp.envelope_level() > 0.3,
            "Envelope should be elevated: {}",
            amp.envelope_level()
        );
    }

    #[test]
    fn test_sag_recovers_after_signal_drops() {
        let params = PowerAmpParams {
            crossover_amount: 0.0,
            sag_depth: 0.5,
            sag_attack_ms: 10.0,
            sag_release_ms: 50.0, // Fast release for test
            transformer_fc: 20000.0,
        };
        let mut amp = PowerAmp::with_params(params, SAMPLE_RATE);

        // First, build up envelope with loud signal
        for _ in 0..4800 {
            amp.process_sample(0.8);
        }
        let envelope_high = amp.envelope_level();

        // Now process silence and let it recover
        for _ in 0..4800 {
            amp.process_sample(0.0);
        }
        let envelope_low = amp.envelope_level();

        assert!(
            envelope_low < envelope_high * 0.3,
            "Envelope should recover: {} -> {}",
            envelope_high,
            envelope_low
        );
    }

    #[test]
    fn test_sag_depth_zero_disables_sag() {
        let params = PowerAmpParams {
            crossover_amount: 0.0,
            sag_depth: 0.0,
            sag_attack_ms: 10.0,
            sag_release_ms: 100.0,
            transformer_fc: 20000.0,
        };
        let mut amp = PowerAmp::with_params(params, SAMPLE_RATE);

        let input = 0.8;

        // Process samples
        for _ in 0..4800 {
            amp.process_sample(input);
        }

        // With sag_depth = 0, output should equal input (minus minimal transformer effect)
        let output = amp.process_sample(input);
        assert!(
            approx_eq(output, input, 0.01),
            "Zero sag depth should not compress: {} vs {}",
            output,
            input
        );
    }

    // ========================================================================
    // Transformer filter tests
    // ========================================================================

    #[test]
    fn test_transformer_rolls_off_high_frequencies() {
        let params = PowerAmpParams {
            crossover_amount: 0.0,
            sag_depth: 0.0,
            transformer_fc: 5000.0, // 5kHz cutoff
            ..Default::default()
        };
        let amp = PowerAmp::with_params(params, SAMPLE_RATE);

        // Check transformer filter response at 10kHz (should be attenuated)
        let response_10k = amp.transformer.magnitude_response(10000.0, SAMPLE_RATE);

        assert!(
            response_10k < 0.5,
            "10kHz should be attenuated with 5kHz cutoff: {}",
            response_10k
        );
    }

    #[test]
    fn test_transformer_passes_low_frequencies() {
        let params = PowerAmpParams {
            crossover_amount: 0.0,
            sag_depth: 0.0,
            transformer_fc: 6000.0,
            ..Default::default()
        };
        let amp = PowerAmp::with_params(params, SAMPLE_RATE);

        // Check transformer filter response at 1kHz (should pass)
        let response_1k = amp.transformer.magnitude_response(1000.0, SAMPLE_RATE);

        assert!(
            approx_eq(response_1k, 1.0, 0.05),
            "1kHz should pass through: {}",
            response_1k
        );
    }

    // ========================================================================
    // Parameter tests
    // ========================================================================

    #[test]
    fn test_parameter_clamping() {
        let params = PowerAmpParams {
            crossover_amount: 0.05,
            sag_depth: 0.5,
            sag_attack_ms: 5.0,   // Below minimum (10)
            sag_release_ms: 600.0, // Above maximum (500)
            transformer_fc: 3000.0, // Below minimum (4000)
        };
        let amp = PowerAmp::with_params(params, SAMPLE_RATE);

        // Transformer should be clamped to minimum 4kHz
        // At 1kHz (well below the clamped 4kHz cutoff), response should be ~unity
        let response_1k = amp.transformer.magnitude_response(1000.0, SAMPLE_RATE);
        assert!(
            response_1k > 0.95,
            "1kHz should pass when fc clamped to 4kHz: {}",
            response_1k
        );

        // At 8kHz (2 octaves above 4kHz cutoff), significant attenuation expected
        let response_8k = amp.transformer.magnitude_response(8000.0, SAMPLE_RATE);
        assert!(
            response_8k < 0.3,
            "8kHz should be attenuated when fc clamped to 4kHz: {}",
            response_8k
        );
    }

    #[test]
    fn test_set_params_updates_processor() {
        let mut amp = PowerAmp::new(SAMPLE_RATE);

        let new_params = PowerAmpParams {
            crossover_amount: 0.08,
            sag_depth: 0.7,
            sag_attack_ms: 50.0,
            sag_release_ms: 300.0,
            transformer_fc: 8000.0,
        };
        amp.set_params(new_params);

        assert!(
            approx_eq(amp.params().crossover_amount, 0.08, EPSILON),
            "Crossover not updated"
        );
        assert!(
            approx_eq(amp.params().sag_depth, 0.7, EPSILON),
            "Sag depth not updated"
        );
    }

    #[test]
    fn test_default_params() {
        let params = PowerAmpParams::default();

        assert!(approx_eq(params.crossover_amount, 0.02, EPSILON));
        assert!(approx_eq(params.sag_depth, 0.3, EPSILON));
        assert!(approx_eq(params.sag_attack_ms, 30.0, EPSILON));
        assert!(approx_eq(params.sag_release_ms, 200.0, EPSILON));
        assert!(approx_eq(params.transformer_fc, 6000.0, EPSILON));
    }

    // ========================================================================
    // Processing tests
    // ========================================================================

    #[test]
    fn test_buffer_processing_matches_sample() {
        let mut amp1 = PowerAmp::new(SAMPLE_RATE);
        let mut amp2 = PowerAmp::new(SAMPLE_RATE);

        let input = [0.1, 0.3, -0.2, 0.5, -0.4, 0.2, 0.0, -0.1];

        // Process sample-by-sample
        let mut output1 = input;
        for sample in output1.iter_mut() {
            *sample = amp1.process_sample(*sample);
        }

        // Process as buffer
        let mut output2 = input;
        amp2.process_buffer(&mut output2);

        // Results should match
        for (a, b) in output1.iter().zip(output2.iter()) {
            assert!(
                approx_eq(*a, *b, EPSILON),
                "Buffer processing mismatch: {} vs {}",
                a,
                b
            );
        }
    }

    #[test]
    fn test_reset_clears_state() {
        let mut amp = PowerAmp::new(SAMPLE_RATE);

        // Build up envelope
        for _ in 0..4800 {
            amp.process_sample(0.8);
        }
        assert!(amp.envelope_level() > 0.1, "Envelope should be elevated");

        // Reset
        amp.reset();

        assert!(
            approx_eq(amp.envelope_level(), 0.0, EPSILON),
            "Envelope should be zero after reset"
        );
    }

    #[test]
    fn test_numerical_stability() {
        let mut amp = PowerAmp::new(SAMPLE_RATE);

        // Process many samples without NaN/Inf
        for i in 0..100000 {
            let input = if i % 2 == 0 { 0.8 } else { -0.8 };
            let output = amp.process_sample(input);

            assert!(
                output.is_finite(),
                "Output became non-finite at sample {}",
                i
            );
            assert!(
                output.abs() <= 1.0,
                "Output exceeded bounds at sample {}: {}",
                i,
                output
            );
        }
    }

    #[test]
    fn test_silence_input() {
        let mut amp = PowerAmp::new(SAMPLE_RATE);

        // Process silence
        for _ in 0..1000 {
            let output = amp.process_sample(0.0);
            assert!(
                approx_eq(output, 0.0, EPSILON),
                "Silence should produce silence: {}",
                output
            );
        }
    }

    // ========================================================================
    // Integration tests
    // ========================================================================

    #[test]
    fn test_full_signal_chain() {
        let params = PowerAmpParams {
            crossover_amount: 0.02,
            sag_depth: 0.3,
            sag_attack_ms: 30.0,
            sag_release_ms: 200.0,
            transformer_fc: 6000.0,
        };
        let mut amp = PowerAmp::with_params(params, SAMPLE_RATE);

        // Generate a simple sine wave
        let freq = 440.0; // A4
        let mut outputs = Vec::new();

        for i in 0..4800 {
            // 100ms
            let t = i as f32 / SAMPLE_RATE;
            let input = 0.5 * libm::sinf(2.0 * core::f32::consts::PI * freq * t);
            let output = amp.process_sample(input);
            outputs.push(output);
        }

        // Verify outputs are reasonable
        for output in &outputs {
            assert!(output.is_finite(), "Output should be finite");
            assert!(
                output.abs() <= 1.0,
                "Output should be bounded: {}",
                output
            );
        }

        // Verify some processing actually happened (not all zeros)
        let max_output = outputs.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
        assert!(max_output > 0.1, "Output should have content: {}", max_output);
    }
}
