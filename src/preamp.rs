//! Preamp Module
//!
//! Models triode vacuum tube gain stages with characteristic nonlinear distortion.
//! Each stage consists of a coupling capacitor, grid conduction limiter, and
//! triode waveshaper that together recreate the harmonic character of real tube preamps.
//!
//! # Overview
//!
//! The preamp is the core of the amplifier's distortion character. This implementation
//! models 1-4 cascaded triode stages, where each stage contributes:
//!
//! - **Coupling capacitor**: High-pass filter removing DC between stages (7-50 Hz cutoff)
//! - **Grid conduction**: Asymmetric soft clipping on positive peaks (tube grid limiting)
//! - **Triode waveshaper**: Asymmetric tanh-based distortion with even harmonic content
//!
//! # Design Philosophy
//!
//! - **Immutable parameters**: Stage parameters set via builder-like pattern
//! - **Functional core**: Pure waveshaping functions separated from stateful filter processing
//! - **No unsafe code**: All operations use safe Rust abstractions
//! - **No heap allocation**: Fixed-size arrays for embedded platform compatibility
//!
//! # Signal Flow
//!
//! ```text
//! Input -> [Stage 1] -> [Stage 2] -> ... -> [Stage N] -> Output
//!
//! Each Stage:
//!   Input -> Coupling Cap (HPF) -> Grid Conduction -> Triode Waveshaper -> Output
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use crate::preamp::{Preamp, StageParams};
//!
//! // Create a 3-stage preamp at 48kHz
//! let mut preamp = Preamp::new(3, 48000.0);
//!
//! // Configure the first stage for high gain
//! preamp.set_stage_params(0, StageParams {
//!     gain: 50.0,
//!     asymmetry: 0.2,
//!     coupling_fc: 15.0,
//!     grid_threshold: 0.7,
//! });
//!
//! // Process audio
//! let output = preamp.process_sample(input);
//! ```
//!
//! # References
//!
//! - `tube_amp_emulation_spec.md` Section 3.3: Preamp Stages
//! - design.md Requirements E3.1-E3.6

use crate::biquad::Biquad;
use crate::dsp_math::tanh_approx;

/// Maximum number of preamp stages supported.
///
/// This limit balances flexibility with embedded memory constraints.
/// Most real tube amps use 2-4 preamp stages.
pub const MAX_STAGES: usize = 4;

/// Minimum gain per stage.
pub const MIN_GAIN: f32 = 1.0;

/// Maximum gain per stage.
pub const MAX_GAIN: f32 = 100.0;

/// Minimum asymmetry value.
pub const MIN_ASYMMETRY: f32 = 0.0;

/// Maximum asymmetry value.
pub const MAX_ASYMMETRY: f32 = 0.5;

/// Minimum coupling capacitor cutoff frequency in Hz.
pub const MIN_COUPLING_FC: f32 = 7.0;

/// Maximum coupling capacitor cutoff frequency in Hz.
pub const MAX_COUPLING_FC: f32 = 50.0;

/// Minimum grid conduction threshold.
pub const MIN_GRID_THRESHOLD: f32 = 0.5;

/// Maximum grid conduction threshold.
pub const MAX_GRID_THRESHOLD: f32 = 1.0;

/// Parameters for a single triode stage.
///
/// These parameters control the distortion character of each preamp stage.
/// All values are clamped to valid ranges when applied.
///
/// # Parameters
///
/// - `gain`: Amplification factor (1.0 to 100.0). Higher values drive the
///   waveshaper harder, producing more distortion.
/// - `asymmetry`: Controls even harmonic content (0.0 to 0.5). Higher values
///   produce more second-order harmonics characteristic of single-ended tube stages.
/// - `coupling_fc`: Coupling capacitor cutoff frequency in Hz (7 to 50 Hz).
///   Lower values allow more bass through between stages.
/// - `grid_threshold`: Grid conduction threshold (0.5 to 1.0). The level at which
///   positive signal peaks start to be soft-limited.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageParams {
    /// Stage gain multiplier (1.0 to 100.0).
    pub gain: f32,
    /// Asymmetry amount for even harmonics (0.0 to 0.5).
    pub asymmetry: f32,
    /// Coupling capacitor high-pass cutoff frequency in Hz (7 to 50).
    pub coupling_fc: f32,
    /// Grid conduction threshold level (0.5 to 1.0).
    pub grid_threshold: f32,
}

impl StageParams {
    /// Creates new stage parameters with all values clamped to valid ranges.
    ///
    /// # Arguments
    ///
    /// * `gain` - Stage gain (clamped to 1.0-100.0)
    /// * `asymmetry` - Asymmetry amount (clamped to 0.0-0.5)
    /// * `coupling_fc` - Coupling cap cutoff in Hz (clamped to 7-50)
    /// * `grid_threshold` - Grid threshold (clamped to 0.5-1.0)
    ///
    /// # Returns
    ///
    /// A `StageParams` with all values clamped to valid ranges.
    #[must_use]
    pub fn new(gain: f32, asymmetry: f32, coupling_fc: f32, grid_threshold: f32) -> Self {
        Self {
            gain: clamp(gain, MIN_GAIN, MAX_GAIN),
            asymmetry: clamp(asymmetry, MIN_ASYMMETRY, MAX_ASYMMETRY),
            coupling_fc: clamp(coupling_fc, MIN_COUPLING_FC, MAX_COUPLING_FC),
            grid_threshold: clamp(grid_threshold, MIN_GRID_THRESHOLD, MAX_GRID_THRESHOLD),
        }
    }

    /// Returns parameters suitable for a clean tone (low gain, low asymmetry).
    #[must_use]
    pub fn clean() -> Self {
        Self {
            gain: 10.0,
            asymmetry: 0.05,
            coupling_fc: 20.0,
            grid_threshold: 0.9,
        }
    }

    /// Returns parameters suitable for a crunch tone (moderate gain).
    #[must_use]
    pub fn crunch() -> Self {
        Self {
            gain: 40.0,
            asymmetry: 0.15,
            coupling_fc: 15.0,
            grid_threshold: 0.7,
        }
    }

    /// Returns parameters suitable for a high-gain lead tone.
    #[must_use]
    pub fn lead() -> Self {
        Self {
            gain: 70.0,
            asymmetry: 0.25,
            coupling_fc: 10.0,
            grid_threshold: 0.6,
        }
    }
}

impl Default for StageParams {
    /// Returns default parameters suitable for moderate overdrive.
    ///
    /// Default values:
    /// - gain: 30.0
    /// - asymmetry: 0.15
    /// - `coupling_fc`: 15.0 Hz
    /// - `grid_threshold`: 0.7
    fn default() -> Self {
        Self {
            gain: 30.0,
            asymmetry: 0.15,
            coupling_fc: 15.0,
            grid_threshold: 0.7,
        }
    }
}

/// Single triode gain stage.
///
/// Implements the complete signal processing chain for one tube stage:
/// coupling capacitor -> grid conduction -> triode waveshaping.
#[derive(Clone)]
struct TriodeStage {
    /// High-pass filter modeling the coupling capacitor between stages.
    coupling_cap: Biquad,
    /// Stage parameters controlling gain and distortion character.
    params: StageParams,
}

impl TriodeStage {
    /// Creates a new triode stage with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `params` - Stage parameters
    /// * `sample_rate` - Audio sample rate in Hz
    ///
    /// # Returns
    ///
    /// A new `TriodeStage` initialized with the given parameters.
    fn new(params: StageParams, sample_rate: f32) -> Self {
        // Q of 0.707 (Butterworth) provides maximally flat passband
        let coupling_cap = Biquad::high_pass(params.coupling_fc, 0.707, sample_rate);
        Self {
            coupling_cap,
            params,
        }
    }

    /// Processes a single sample through the complete stage.
    ///
    /// Signal flow:
    /// 1. Coupling capacitor (DC blocking between stages)
    /// 2. Grid conduction (asymmetric limiting on positive peaks)
    /// 3. Triode waveshaper (asymmetric tanh distortion)
    ///
    /// # Arguments
    ///
    /// * `x` - Input sample
    ///
    /// # Returns
    ///
    /// Processed output sample.
    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        // 1. Coupling capacitor removes DC offset from previous stage
        let coupled = self.coupling_cap.process_sample(x);

        // 2. Grid conduction limits positive peaks (asymmetric clipping)
        let grid = grid_conduct(coupled, self.params.grid_threshold);

        // 3. Triode waveshaper applies tube-like distortion
        triode_waveshape(grid, self.params.gain, self.params.asymmetry)
    }

    /// Resets the stage's internal filter state.
    fn reset(&mut self) {
        self.coupling_cap.reset();
    }
}

/// Grid conduction limiter - asymmetric soft clip on positive signal.
///
/// Models the behavior of a vacuum tube's control grid, which conducts
/// when the grid voltage becomes positive relative to the cathode.
/// This creates asymmetric clipping that adds even-order harmonics.
///
/// # Arguments
///
/// * `x` - Input signal value
/// * `threshold` - Level at which soft limiting begins (0.5 to 1.0)
///
/// # Returns
///
/// Signal with positive peaks soft-limited above the threshold.
///
/// # Algorithm
///
/// - Below threshold: signal passes unchanged
/// - Above threshold: excess is compressed by factor of 0.1
///
/// ```text
/// if x > threshold:
///     output = threshold + (x - threshold) * 0.1
/// else:
///     output = x
/// ```
#[inline]
#[must_use]
pub fn grid_conduct(x: f32, threshold: f32) -> f32 {
    if x > threshold {
        threshold + (x - threshold) * 0.1
    } else {
        x
    }
}

/// Triode waveshaper with asymmetric distortion.
///
/// Models the transfer characteristic of a triode vacuum tube using a
/// combination of symmetric tanh saturation and asymmetric harmonic generation.
///
/// # Arguments
///
/// * `x` - Input signal value
/// * `drive` - Gain/drive amount (1.0 to 100.0)
/// * `asymmetry` - Amount of asymmetric (even harmonic) distortion (0.0 to 0.5)
///
/// # Returns
///
/// Waveshaped output with tube-like harmonic content.
///
/// # Algorithm
///
/// ```text
/// driven = x * drive
/// symmetric = tanh(driven)           // Odd harmonics (3rd, 5th, ...)
/// asymmetric = symmetric^2 * asymmetry  // Even harmonics (2nd, 4th, ...)
/// output = symmetric + asymmetric
/// ```
///
/// The symmetric component provides the core saturation character (odd harmonics),
/// while the asymmetric component adds the even-order harmonics that give tubes
/// their characteristic warmth.
#[inline]
#[must_use]
pub fn triode_waveshape(x: f32, drive: f32, asymmetry: f32) -> f32 {
    let driven = x * drive;
    let symmetric = tanh_approx(driven);
    let asymmetric = symmetric * symmetric * asymmetry;
    symmetric + asymmetric
}

/// Multi-stage preamp processor.
///
/// Contains 1-4 cascaded triode stages, each contributing gain and distortion.
/// More stages and higher gain settings result in more distortion and compression.
///
/// # Design Notes
///
/// - Stages are stored as `Option<TriodeStage>` to allow dynamic stage count
/// - Only active stages (up to `num_stages`) are processed
/// - The fixed-size array avoids heap allocation for embedded use
pub struct Preamp {
    /// Array of optional triode stages.
    stages: [Option<TriodeStage>; MAX_STAGES],
    /// Number of active stages (1 to MAX_STAGES).
    num_stages: usize,
    /// Sample rate for filter coefficient calculation.
    sample_rate: f32,
}

impl Preamp {
    /// Creates a new preamp with the specified number of stages.
    ///
    /// All stages are initialized with default parameters.
    ///
    /// # Arguments
    ///
    /// * `num_stages` - Number of stages (clamped to 1-4)
    /// * `sample_rate` - Audio sample rate in Hz
    ///
    /// # Returns
    ///
    /// A new `Preamp` with the specified configuration.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut preamp = Preamp::new(3, 48000.0);
    /// ```
    #[must_use]
    pub fn new(num_stages: usize, sample_rate: f32) -> Self {
        let num = num_stages.clamp(1, MAX_STAGES);
        let mut preamp = Self {
            stages: [None, None, None, None],
            num_stages: num,
            sample_rate,
        };
        // Initialize all active stages with default parameters
        for i in 0..num {
            preamp.stages[i] = Some(TriodeStage::new(StageParams::default(), sample_rate));
        }
        preamp
    }

    /// Creates a preamp configured for clean tones.
    ///
    /// Uses 2 stages with clean parameters.
    #[must_use]
    pub fn clean(sample_rate: f32) -> Self {
        let mut preamp = Self::new(2, sample_rate);
        let params = StageParams::clean();
        preamp.set_stage_params(0, params);
        preamp.set_stage_params(1, params);
        preamp
    }

    /// Creates a preamp configured for crunch tones.
    ///
    /// Uses 3 stages with crunch parameters.
    #[must_use]
    pub fn crunch(sample_rate: f32) -> Self {
        let mut preamp = Self::new(3, sample_rate);
        let params = StageParams::crunch();
        for i in 0..3 {
            preamp.set_stage_params(i, params);
        }
        preamp
    }

    /// Creates a preamp configured for high-gain lead tones.
    ///
    /// Uses 4 stages with lead parameters.
    #[must_use]
    pub fn lead(sample_rate: f32) -> Self {
        let mut preamp = Self::new(4, sample_rate);
        let params = StageParams::lead();
        for i in 0..4 {
            preamp.set_stage_params(i, params);
        }
        preamp
    }

    /// Sets the number of active stages.
    ///
    /// If increasing the stage count, new stages are initialized with
    /// default parameters. Existing stages retain their parameters.
    ///
    /// # Arguments
    ///
    /// * `num` - New stage count (clamped to 1-4)
    pub fn set_num_stages(&mut self, num: usize) {
        self.num_stages = num.clamp(1, MAX_STAGES);
        // Initialize any new stages that don't exist yet
        for i in 0..self.num_stages {
            if self.stages[i].is_none() {
                self.stages[i] = Some(TriodeStage::new(StageParams::default(), self.sample_rate));
            }
        }
    }

    /// Sets parameters for a specific stage.
    ///
    /// Creates a new stage with the given parameters. The stage's coupling
    /// capacitor filter is recalculated based on the new cutoff frequency.
    ///
    /// # Arguments
    ///
    /// * `stage_idx` - Stage index (0 to num_stages-1)
    /// * `params` - New stage parameters
    ///
    /// # Note
    ///
    /// If `stage_idx` is >= `num_stages`, this call has no effect.
    pub fn set_stage_params(&mut self, stage_idx: usize, params: StageParams) {
        if stage_idx < self.num_stages {
            self.stages[stage_idx] = Some(TriodeStage::new(params, self.sample_rate));
        }
    }

    /// Gets parameters for a specific stage.
    ///
    /// # Arguments
    ///
    /// * `stage_idx` - Stage index (0 to num_stages-1)
    ///
    /// # Returns
    ///
    /// Some(params) if the stage exists, None otherwise.
    #[must_use]
    pub fn get_stage_params(&self, stage_idx: usize) -> Option<StageParams> {
        if stage_idx < self.num_stages {
            self.stages[stage_idx].as_ref().map(|s| s.params)
        } else {
            None
        }
    }

    /// Processes a single sample through all active stages.
    ///
    /// The sample passes through each stage in sequence, with each stage
    /// adding its characteristic distortion and filtering.
    ///
    /// # Arguments
    ///
    /// * `x` - Input sample
    ///
    /// # Returns
    ///
    /// Processed output sample.
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let mut y = x;
        for i in 0..self.num_stages {
            if let Some(ref mut stage) = self.stages[i] {
                y = stage.process(y);
            }
        }
        y
    }

    /// Processes a buffer of samples in place.
    ///
    /// More efficient than calling `process_sample` in a loop due to
    /// better instruction cache utilization.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Audio buffer to process in place
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Resets all stage filter states.
    ///
    /// Call this when switching presets or starting a new audio stream
    /// to prevent filter state artifacts.
    pub fn reset(&mut self) {
        for stage in self.stages.iter_mut().flatten() {
            stage.reset();
        }
    }

    /// Returns the number of active stages.
    #[must_use]
    pub fn num_stages(&self) -> usize {
        self.num_stages
    }

    /// Returns the sample rate.
    #[must_use]
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }
}

/// Clamp a value to a specified range.
#[inline]
fn clamp(x: f32, min: f32, max: f32) -> f32 {
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tolerance for floating-point comparisons.
    const EPSILON: f32 = 1e-5;

    /// Helper function to check if two floats are approximately equal.
    fn approx_eq(a: f32, b: f32, epsilon: f32) -> bool {
        (a - b).abs() < epsilon
    }

    // ========================================================================
    // Grid Conduction Tests
    // ========================================================================

    #[test]
    fn test_grid_conduct_below_threshold() {
        // Below threshold, signal passes unchanged
        let threshold = 0.7;
        let input = 0.5;
        let output = grid_conduct(input, threshold);
        assert!(
            approx_eq(output, input, EPSILON),
            "Below threshold: expected {}, got {}",
            input,
            output
        );
    }

    #[test]
    fn test_grid_conduct_at_threshold() {
        // At threshold, output equals threshold
        let threshold = 0.7;
        let output = grid_conduct(threshold, threshold);
        assert!(
            approx_eq(output, threshold, EPSILON),
            "At threshold: expected {}, got {}",
            threshold,
            output
        );
    }

    #[test]
    fn test_grid_conduct_above_threshold() {
        // Above threshold, excess is compressed by 0.1
        let threshold = 0.7;
        let input = 1.0;
        let expected = threshold + (input - threshold) * 0.1; // 0.7 + 0.3 * 0.1 = 0.73
        let output = grid_conduct(input, threshold);
        assert!(
            approx_eq(output, expected, EPSILON),
            "Above threshold: expected {}, got {}",
            expected,
            output
        );
    }

    #[test]
    fn test_grid_conduct_limits_positive_peaks() {
        // Large positive input is significantly limited
        let threshold = 0.7;
        let input = 2.0;
        let output = grid_conduct(input, threshold);

        // Output should be much less than input
        assert!(
            output < input,
            "Grid conduct should limit positive peaks: {} >= {}",
            output,
            input
        );
        // But greater than threshold
        assert!(
            output > threshold,
            "Output should be above threshold: {} <= {}",
            output,
            threshold
        );
    }

    #[test]
    fn test_grid_conduct_negative_passthrough() {
        // Negative values pass through unchanged (asymmetric behavior)
        let threshold = 0.7;
        let input = -1.0;
        let output = grid_conduct(input, threshold);
        assert!(
            approx_eq(output, input, EPSILON),
            "Negative values should pass unchanged: expected {}, got {}",
            input,
            output
        );
    }

    // ========================================================================
    // Triode Waveshaper Tests
    // ========================================================================

    #[test]
    fn test_triode_waveshape_zero_input() {
        // Zero input should produce zero output
        let output = triode_waveshape(0.0, 30.0, 0.15);
        assert!(
            approx_eq(output, 0.0, EPSILON),
            "Zero input should give zero output: got {}",
            output
        );
    }

    #[test]
    fn test_triode_waveshape_unity_gain() {
        // At unity gain, small signals pass roughly unchanged
        let input = 0.1;
        let output = triode_waveshape(input, 1.0, 0.0);
        // tanh_approx(0.1) ~= 0.0999
        assert!(
            (output - input).abs() < 0.01,
            "Unity gain should preserve small signals: input {}, output {}",
            input,
            output
        );
    }

    #[test]
    fn test_triode_waveshape_saturation() {
        // High gain causes saturation toward +/- 1.0
        let output_pos = triode_waveshape(0.5, 50.0, 0.0);
        let output_neg = triode_waveshape(-0.5, 50.0, 0.0);

        // Positive input saturates toward 1.0
        assert!(
            output_pos > 0.9,
            "High drive should saturate positive: got {}",
            output_pos
        );
        // Negative input saturates toward -1.0
        assert!(
            output_neg < -0.9,
            "High drive should saturate negative: got {}",
            output_neg
        );
    }

    #[test]
    fn test_triode_waveshape_asymmetry_effect() {
        // Asymmetry adds DC offset due to squared term
        let input = 0.5;
        let drive = 20.0;

        let symmetric_output = triode_waveshape(input, drive, 0.0);
        let asymmetric_output = triode_waveshape(input, drive, 0.3);

        // Asymmetric output should be larger (squared term adds positive value)
        assert!(
            asymmetric_output > symmetric_output,
            "Asymmetry should increase positive output: symmetric {}, asymmetric {}",
            symmetric_output,
            asymmetric_output
        );
    }

    #[test]
    fn test_triode_waveshape_asymmetric_harmonics() {
        // Asymmetry creates even harmonics (DC offset difference between + and -)
        let drive = 20.0;
        let asymmetry = 0.3;

        let pos_output = triode_waveshape(0.5, drive, asymmetry);
        let neg_output = triode_waveshape(-0.5, drive, asymmetry);

        // With asymmetry, |positive output| != |negative output|
        assert!(
            (pos_output.abs() - neg_output.abs()).abs() > 0.01,
            "Asymmetry should create different positive/negative magnitudes: +{}, -{}",
            pos_output,
            neg_output
        );
    }

    #[test]
    fn test_triode_waveshape_no_asymmetry_is_odd() {
        // Without asymmetry, waveshaper is an odd function
        let drive = 30.0;
        let asymmetry = 0.0;

        let pos = triode_waveshape(0.5, drive, asymmetry);
        let neg = triode_waveshape(-0.5, drive, asymmetry);

        assert!(
            approx_eq(pos, -neg, EPSILON),
            "Without asymmetry, f(x) = -f(-x): got f(0.5)={}, f(-0.5)={}",
            pos,
            neg
        );
    }

    // ========================================================================
    // StageParams Tests
    // ========================================================================

    #[test]
    fn test_stage_params_default() {
        let params = StageParams::default();
        assert!(approx_eq(params.gain, 30.0, EPSILON));
        assert!(approx_eq(params.asymmetry, 0.15, EPSILON));
        assert!(approx_eq(params.coupling_fc, 15.0, EPSILON));
        assert!(approx_eq(params.grid_threshold, 0.7, EPSILON));
    }

    #[test]
    fn test_stage_params_new_clamping() {
        // Values outside range should be clamped
        let params = StageParams::new(200.0, 1.0, 100.0, 0.0);
        assert!(approx_eq(params.gain, MAX_GAIN, EPSILON));
        assert!(approx_eq(params.asymmetry, MAX_ASYMMETRY, EPSILON));
        assert!(approx_eq(params.coupling_fc, MAX_COUPLING_FC, EPSILON));
        assert!(approx_eq(params.grid_threshold, MIN_GRID_THRESHOLD, EPSILON));

        let params2 = StageParams::new(0.0, -1.0, 0.0, 2.0);
        assert!(approx_eq(params2.gain, MIN_GAIN, EPSILON));
        assert!(approx_eq(params2.asymmetry, MIN_ASYMMETRY, EPSILON));
        assert!(approx_eq(params2.coupling_fc, MIN_COUPLING_FC, EPSILON));
        assert!(approx_eq(params2.grid_threshold, MAX_GRID_THRESHOLD, EPSILON));
    }

    #[test]
    fn test_stage_params_presets() {
        let clean = StageParams::clean();
        let crunch = StageParams::crunch();
        let lead = StageParams::lead();

        // Clean has lowest gain
        assert!(clean.gain < crunch.gain);
        assert!(crunch.gain < lead.gain);

        // Lead has lowest grid threshold (clips sooner)
        assert!(lead.grid_threshold < crunch.grid_threshold);
        assert!(crunch.grid_threshold < clean.grid_threshold);
    }

    // ========================================================================
    // Preamp Tests
    // ========================================================================

    #[test]
    fn test_preamp_creation() {
        let preamp = Preamp::new(3, 48000.0);
        assert_eq!(preamp.num_stages(), 3);
        assert!(approx_eq(preamp.sample_rate(), 48000.0, EPSILON));
    }

    #[test]
    fn test_preamp_stage_count_clamping() {
        // Stage count should be clamped to 1-4
        let preamp_min = Preamp::new(0, 48000.0);
        assert_eq!(preamp_min.num_stages(), 1);

        let preamp_max = Preamp::new(10, 48000.0);
        assert_eq!(preamp_max.num_stages(), MAX_STAGES);
    }

    #[test]
    fn test_preamp_set_num_stages() {
        let mut preamp = Preamp::new(2, 48000.0);
        assert_eq!(preamp.num_stages(), 2);

        preamp.set_num_stages(4);
        assert_eq!(preamp.num_stages(), 4);

        preamp.set_num_stages(1);
        assert_eq!(preamp.num_stages(), 1);
    }

    #[test]
    fn test_preamp_set_stage_params() {
        let mut preamp = Preamp::new(2, 48000.0);

        let params = StageParams::new(50.0, 0.25, 20.0, 0.8);
        preamp.set_stage_params(0, params);

        let retrieved = preamp.get_stage_params(0).unwrap();
        assert!(approx_eq(retrieved.gain, 50.0, EPSILON));
        assert!(approx_eq(retrieved.asymmetry, 0.25, EPSILON));
    }

    #[test]
    fn test_preamp_get_stage_params_out_of_bounds() {
        let preamp = Preamp::new(2, 48000.0);
        assert!(preamp.get_stage_params(5).is_none());
    }

    #[test]
    fn test_preamp_single_stage_processing() {
        let mut preamp = Preamp::new(1, 48000.0);
        preamp.set_stage_params(
            0,
            StageParams {
                gain: 10.0,
                asymmetry: 0.0,
                coupling_fc: 15.0,
                grid_threshold: 1.0,
            },
        );

        // Small input should produce output
        let input = 0.1;
        // Need to process multiple samples to warm up filter state
        for _ in 0..1000 {
            preamp.process_sample(input);
        }
        let output = preamp.process_sample(input);

        // Output should be non-zero and related to gain
        assert!(
            output.abs() > 0.0,
            "Single stage should produce output: got {}",
            output
        );
    }

    #[test]
    fn test_preamp_more_stages_more_distortion() {
        // More stages should produce more distortion/compression
        let sample_rate = 48000.0;
        let input = 0.3;

        // Create preamps with different stage counts
        let mut preamp1 = Preamp::new(1, sample_rate);
        let mut preamp2 = Preamp::new(2, sample_rate);
        let mut preamp4 = Preamp::new(4, sample_rate);

        // Warm up filters and get steady-state response
        for _ in 0..5000 {
            preamp1.process_sample(input);
            preamp2.process_sample(input);
            preamp4.process_sample(input);
        }

        let out1 = preamp1.process_sample(input);
        let out2 = preamp2.process_sample(input);
        let out4 = preamp4.process_sample(input);

        // More stages should produce higher output (more saturation toward 1.0)
        // Due to cascaded gain, 4 stages should produce output closer to saturation
        assert!(
            out2.abs() > out1.abs() * 0.9, // Allow some tolerance due to filter effects
            "2 stages should produce at least as much as 1 stage: 1={}, 2={}",
            out1,
            out2
        );
        assert!(
            out4.abs() > out2.abs() * 0.9,
            "4 stages should produce at least as much as 2 stages: 2={}, 4={}",
            out2,
            out4
        );
    }

    #[test]
    fn test_preamp_coupling_cap_removes_dc() {
        let mut preamp = Preamp::new(2, 48000.0);

        // Feed DC signal (constant value)
        let dc_input = 0.5;

        // After many samples, coupling cap should remove DC
        let mut output = 0.0;
        for _ in 0..48000 {
            // 1 second at 48kHz
            output = preamp.process_sample(dc_input);
        }

        // Output should be near zero (DC removed between stages)
        // Note: First stage receives DC, but coupling caps between stages remove it
        assert!(
            output.abs() < 0.1,
            "DC should be significantly attenuated: got {}",
            output
        );
    }

    #[test]
    fn test_preamp_buffer_processing() {
        let mut preamp1 = Preamp::new(2, 48000.0);
        let mut preamp2 = Preamp::new(2, 48000.0);

        let input = [0.1, 0.2, 0.3, 0.2, 0.1, 0.0, -0.1, -0.2];

        // Process sample-by-sample
        let mut output1 = input;
        for sample in output1.iter_mut() {
            *sample = preamp1.process_sample(*sample);
        }

        // Process as buffer
        let mut output2 = input;
        preamp2.process_buffer(&mut output2);

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
    fn test_preamp_reset() {
        let mut preamp = Preamp::new(2, 48000.0);

        // Build up filter state
        for _ in 0..1000 {
            preamp.process_sample(0.5);
        }

        // Reset
        preamp.reset();

        // After reset, processing a zero should give near-zero output
        let output = preamp.process_sample(0.0);
        assert!(
            output.abs() < EPSILON,
            "After reset, zero input should give near-zero output: got {}",
            output
        );
    }

    #[test]
    fn test_preamp_factory_presets() {
        let clean = Preamp::clean(48000.0);
        let crunch = Preamp::crunch(48000.0);
        let lead = Preamp::lead(48000.0);

        assert_eq!(clean.num_stages(), 2);
        assert_eq!(crunch.num_stages(), 3);
        assert_eq!(lead.num_stages(), 4);
    }

    #[test]
    fn test_preamp_parameter_per_stage() {
        let mut preamp = Preamp::new(4, 48000.0);

        // Set different parameters for each stage
        preamp.set_stage_params(0, StageParams::new(10.0, 0.1, 10.0, 0.9));
        preamp.set_stage_params(1, StageParams::new(30.0, 0.2, 15.0, 0.8));
        preamp.set_stage_params(2, StageParams::new(50.0, 0.3, 20.0, 0.7));
        preamp.set_stage_params(3, StageParams::new(70.0, 0.4, 25.0, 0.6));

        // Verify each stage has its own parameters
        let p0 = preamp.get_stage_params(0).unwrap();
        let p1 = preamp.get_stage_params(1).unwrap();
        let p2 = preamp.get_stage_params(2).unwrap();
        let p3 = preamp.get_stage_params(3).unwrap();

        assert!(approx_eq(p0.gain, 10.0, EPSILON));
        assert!(approx_eq(p1.gain, 30.0, EPSILON));
        assert!(approx_eq(p2.gain, 50.0, EPSILON));
        assert!(approx_eq(p3.gain, 70.0, EPSILON));
    }

    #[test]
    fn test_preamp_numerical_stability() {
        let mut preamp = Preamp::new(4, 48000.0);

        // Process many samples with varying input
        for i in 0..100000 {
            let input = if i % 2 == 0 { 0.5 } else { -0.5 };
            let output = preamp.process_sample(input);
            assert!(
                output.is_finite(),
                "Output became non-finite at sample {}",
                i
            );
        }
    }

    #[test]
    fn test_preamp_high_gain_output_is_finite() {
        let mut preamp = Preamp::new(4, 48000.0);

        // Set high (but realistic) gain on all stages
        // Note: At maximum gain (100.0) per stage, cascaded gain explodes
        // Real preamps would use more moderate per-stage gain
        let high_params = StageParams::new(50.0, MAX_ASYMMETRY, 15.0, MIN_GRID_THRESHOLD);
        for i in 0..4 {
            preamp.set_stage_params(i, high_params);
        }

        // Process samples and ensure output remains finite
        for i in 0..1000 {
            let input = if i % 2 == 0 { 0.1 } else { -0.1 };
            let output = preamp.process_sample(input);
            assert!(
                output.is_finite(),
                "Output became non-finite at sample {}",
                i
            );
        }
    }

    #[test]
    fn test_preamp_extreme_gain_still_finite() {
        let mut preamp = Preamp::new(4, 48000.0);

        // Maximum gain creates astronomical values but should remain finite
        let max_params = StageParams::new(MAX_GAIN, MAX_ASYMMETRY, 15.0, MIN_GRID_THRESHOLD);
        for i in 0..4 {
            preamp.set_stage_params(i, max_params);
        }

        // With extreme settings, output will be huge but should stay finite
        for _ in 0..100 {
            let output = preamp.process_sample(0.01);
            assert!(output.is_finite(), "Output became infinite");
        }
    }
}
