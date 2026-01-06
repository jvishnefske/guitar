//! Output Stage Module
//!
//! Provides master volume control and soft limiting to prevent digital clipping.
//!
//! # Overview
//!
//! The output stage is the final processing block in the signal chain, responsible for:
//! - Master volume control with smooth dB-based adjustment
//! - Soft clipping to prevent harsh digital overs
//! - Output ceiling enforcement at configurable threshold
//!
//! # Design Requirements (E7)
//!
//! From `design.md` section E7:
//! - E7.1: Master volume: 0 to -60dB
//! - E7.2: Soft clipper to prevent digital overs
//! - E7.3: Output ceiling at 0.8 threshold
//!
//! # Example
//!
//! ```
//! use guitar_amp_dsp::output::OutputStage;
//!
//! let mut output = OutputStage::new();
//! output.set_volume_db(-6.0);  // ~0.5 linear gain
//! output.set_ceiling(0.8);     // Limit peaks to 0.8
//!
//! let input = 1.2;  // Hot signal
//! let limited = output.process_sample(input);
//! assert!(limited <= 0.8);
//! ```

use crate::dsp_math::{db_to_linear, soft_clip};

/// Output stage processor with volume control and soft limiting.
///
/// Implements the final stage of the amp signal chain, providing master
/// volume attenuation and soft clipping to prevent digital overs.
///
/// # Fields
///
/// - `volume`: Linear gain multiplier (0.0 to 1.0), derived from dB setting
/// - `ceiling`: Maximum output level for soft clipper (default 1.0)
///
/// # Immutability
///
/// Following the project's architectural principles, the `OutputStage`
/// uses simple scalar fields rather than complex object graphs. State
/// transitions occur through explicit setter methods that validate inputs.
pub struct OutputStage {
    /// Linear gain (0.0 to 1.0), computed from dB volume setting
    volume: f32,
    /// Soft clip ceiling (default 1.0)
    ceiling: f32,
}

impl OutputStage {
    /// Create a new output stage with unity gain.
    ///
    /// # Returns
    ///
    /// Output stage with:
    /// - Volume at 0dB (unity gain, linear 1.0)
    /// - Ceiling at 1.0 (full scale)
    ///
    /// # Example
    ///
    /// ```
    /// use guitar_amp_dsp::output::OutputStage;
    ///
    /// let output = OutputStage::new();
    /// // Passes signal unchanged when below soft clip threshold
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            volume: 1.0,
            ceiling: 1.0,
        }
    }

    /// Set master volume in decibels.
    ///
    /// # Arguments
    ///
    /// * `volume_db` - Volume in dB, clamped to -60 to 0 dB range
    ///
    /// # Behavior
    ///
    /// - 0 dB: Unity gain (1.0 linear)
    /// - -6 dB: Approximately half amplitude (~0.5 linear)
    /// - -20 dB: 0.1 linear
    /// - -60 dB: Effectively silent (~0.001 linear)
    ///
    /// Values outside the -60 to 0 dB range are clamped to prevent
    /// unexpected behavior (no boost allowed, minimum -60dB).
    ///
    /// # Example
    ///
    /// ```
    /// use guitar_amp_dsp::output::OutputStage;
    ///
    /// let mut output = OutputStage::new();
    /// output.set_volume_db(-6.0);  // Half volume
    /// output.set_volume_db(-120.0); // Clamped to -60dB
    /// output.set_volume_db(10.0);   // Clamped to 0dB (no boost)
    /// ```
    pub fn set_volume_db(&mut self, volume_db: f32) {
        let clamped = volume_db.clamp(-60.0, 0.0);
        self.volume = db_to_linear(clamped);
    }

    /// Get the current volume as linear gain.
    ///
    /// # Returns
    ///
    /// Linear gain value in range 0.001 to 1.0
    #[must_use]
    pub fn volume_linear(&self) -> f32 {
        self.volume
    }

    /// Set the soft clip ceiling.
    ///
    /// # Arguments
    ///
    /// * `ceiling` - Maximum output level (default 1.0)
    ///
    /// # Behavior
    ///
    /// The ceiling value is clamped to a minimum of 0.1 to prevent
    /// division issues in the soft clip algorithm. The soft clipper
    /// uses an 80% threshold, meaning signals below `0.8 * ceiling`
    /// pass unchanged, while signals above are smoothly compressed.
    ///
    /// # Example
    ///
    /// ```
    /// use guitar_amp_dsp::output::OutputStage;
    ///
    /// let mut output = OutputStage::new();
    /// output.set_ceiling(0.8);  // Limit output to 0.8 max
    /// ```
    pub fn set_ceiling(&mut self, ceiling: f32) {
        self.ceiling = ceiling.max(0.1); // Prevent division issues
    }

    /// Get the current ceiling value.
    ///
    /// # Returns
    ///
    /// Current soft clip ceiling
    #[must_use]
    pub fn ceiling(&self) -> f32 {
        self.ceiling
    }

    /// Process a single sample.
    ///
    /// Applies volume scaling followed by soft clipping.
    ///
    /// # Arguments
    ///
    /// * `x` - Input sample
    ///
    /// # Returns
    ///
    /// Output sample with volume applied and soft clipping enforced
    ///
    /// # Performance
    ///
    /// This method is marked `#[inline]` for optimal performance in
    /// tight sample-processing loops.
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let scaled = x * self.volume;
        soft_clip(scaled, self.ceiling)
    }

    /// Process a buffer of samples in place.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Mutable slice of samples to process
    ///
    /// # Example
    ///
    /// ```
    /// use guitar_amp_dsp::output::OutputStage;
    ///
    /// let mut output = OutputStage::new();
    /// output.set_volume_db(-6.0);
    ///
    /// let mut buffer = [0.5, 0.8, 1.2, -0.9];
    /// output.process_buffer(&mut buffer);
    /// // All samples now have volume applied and soft clipping
    /// ```
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }
}

impl Default for OutputStage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    fn approx_eq(a: f32, b: f32, epsilon: f32) -> bool {
        (a - b).abs() < epsilon
    }

    // =========================================================================
    // Volume Tests (E7.1)
    // =========================================================================

    #[test]
    fn test_new_has_unity_gain() {
        let output = OutputStage::new();
        assert!(approx_eq(output.volume_linear(), 1.0, EPSILON));
    }

    #[test]
    fn test_default_has_unity_gain() {
        let output = OutputStage::default();
        assert!(approx_eq(output.volume_linear(), 1.0, EPSILON));
    }

    #[test]
    fn test_volume_0db_unity() {
        let mut output = OutputStage::new();
        output.set_volume_db(0.0);
        assert!(approx_eq(output.volume_linear(), 1.0, EPSILON));
    }

    #[test]
    fn test_volume_minus_6db_half() {
        let mut output = OutputStage::new();
        output.set_volume_db(-6.0);
        // -6dB ~= 0.501187
        assert!(approx_eq(output.volume_linear(), 0.501187, 0.001));
    }

    #[test]
    fn test_volume_minus_20db() {
        let mut output = OutputStage::new();
        output.set_volume_db(-20.0);
        // -20dB = 0.1
        assert!(approx_eq(output.volume_linear(), 0.1, EPSILON));
    }

    #[test]
    fn test_volume_minus_60db_very_quiet() {
        let mut output = OutputStage::new();
        output.set_volume_db(-60.0);
        // -60dB = 0.001
        assert!(approx_eq(output.volume_linear(), 0.001, EPSILON));
    }

    #[test]
    fn test_volume_clamps_positive_to_zero() {
        let mut output = OutputStage::new();
        output.set_volume_db(10.0); // Should clamp to 0dB
        assert!(approx_eq(output.volume_linear(), 1.0, EPSILON));
    }

    #[test]
    fn test_volume_clamps_below_minus_60() {
        let mut output = OutputStage::new();
        output.set_volume_db(-100.0); // Should clamp to -60dB
        assert!(approx_eq(output.volume_linear(), 0.001, EPSILON));
    }

    // =========================================================================
    // Soft Clipping Tests (E7.2, E7.3)
    // =========================================================================

    #[test]
    fn test_soft_clip_below_threshold_passes() {
        let mut output = OutputStage::new();
        output.set_ceiling(1.0);
        // Threshold is 80% of ceiling = 0.8
        // Signal at 0.5 should pass unchanged
        let result = output.process_sample(0.5);
        assert!(approx_eq(result, 0.5, EPSILON));
    }

    #[test]
    fn test_soft_clip_at_threshold() {
        let mut output = OutputStage::new();
        output.set_ceiling(1.0);
        // At exactly 0.8, minimal compression
        let result = output.process_sample(0.8);
        assert!(approx_eq(result, 0.8, 0.001));
    }

    #[test]
    fn test_soft_clip_above_ceiling_is_limited() {
        let mut output = OutputStage::new();
        output.set_ceiling(1.0);
        // Input of 1.5 should be compressed below ceiling
        let result = output.process_sample(1.5);
        assert!(result < 1.0);
        assert!(result > 0.8); // Between threshold and ceiling
    }

    #[test]
    fn test_soft_clip_extreme_input_never_exceeds_ceiling() {
        let mut output = OutputStage::new();
        output.set_ceiling(1.0);
        // Even with extreme input, output approaches but never exceeds ceiling
        let result = output.process_sample(100.0);
        assert!(result < 1.0);
    }

    #[test]
    fn test_soft_clip_negative_signal() {
        let mut output = OutputStage::new();
        output.set_ceiling(1.0);
        let result = output.process_sample(-1.5);
        assert!(result > -1.0);
        assert!(result < -0.8);
    }

    #[test]
    fn test_output_never_exceeds_ceiling_080() {
        // E7.3: Output ceiling at 0.8 threshold
        let mut output = OutputStage::new();
        output.set_ceiling(0.8);

        // Test various hot signals
        let test_inputs = [0.9, 1.0, 1.5, 2.0, 5.0, 10.0];
        for input in test_inputs {
            let result = output.process_sample(input);
            assert!(
                result < 0.8,
                "Input {} produced output {} which exceeds ceiling 0.8",
                input,
                result
            );
        }

        // Test negative signals too
        for input in test_inputs {
            let result = output.process_sample(-input);
            assert!(
                result > -0.8,
                "Input {} produced output {} which exceeds ceiling -0.8",
                -input,
                result
            );
        }
    }

    #[test]
    fn test_ceiling_minimum_enforced() {
        let mut output = OutputStage::new();
        output.set_ceiling(0.05); // Below minimum
        assert!(output.ceiling() >= 0.1);
    }

    // =========================================================================
    // Combined Volume + Clipping Tests
    // =========================================================================

    #[test]
    fn test_volume_applied_before_clipping() {
        let mut output = OutputStage::new();
        output.set_volume_db(-6.0); // ~0.5 gain
        output.set_ceiling(1.0);

        // Input of 1.0 * 0.5 = 0.5, below threshold, passes unchanged
        let result = output.process_sample(1.0);
        assert!(approx_eq(result, 0.501187, 0.001));
    }

    #[test]
    fn test_hot_signal_with_volume_reduction() {
        let mut output = OutputStage::new();
        output.set_volume_db(-6.0); // ~0.5 gain
        output.set_ceiling(1.0);

        // Input of 2.0 * 0.5 = 1.0, above threshold 0.8
        let result = output.process_sample(2.0);
        assert!(result < 1.0);
        assert!(result > 0.8);
    }

    // =========================================================================
    // Buffer Processing Tests
    // =========================================================================

    #[test]
    fn test_process_buffer_applies_to_all_samples() {
        let mut output = OutputStage::new();
        output.set_volume_db(-6.0); // ~0.5 gain

        let mut buffer = [0.5, 0.8, 1.0, -0.5];
        output.process_buffer(&mut buffer);

        // All samples should be scaled by ~0.5
        assert!(approx_eq(buffer[0], 0.25, 0.01));
        assert!(approx_eq(buffer[1], 0.4, 0.01));
        assert!(approx_eq(buffer[2], 0.5, 0.01));
        assert!(approx_eq(buffer[3], -0.25, 0.01));
    }

    #[test]
    fn test_process_buffer_soft_clips_hot_samples() {
        let mut output = OutputStage::new();
        output.set_ceiling(0.8);

        let mut buffer = [0.5, 1.0, 1.5, -1.2];
        output.process_buffer(&mut buffer);

        // 0.5 below threshold (0.64), passes
        assert!(approx_eq(buffer[0], 0.5, EPSILON));
        // 1.0, 1.5, -1.2 above threshold, clipped
        assert!(buffer[1] < 0.8);
        assert!(buffer[2] < 0.8);
        assert!(buffer[3] > -0.8);
    }

    #[test]
    fn test_process_empty_buffer() {
        let mut output = OutputStage::new();
        let mut buffer: [f32; 0] = [];
        output.process_buffer(&mut buffer);
        // Should not panic
    }

    // =========================================================================
    // Getter Tests
    // =========================================================================

    #[test]
    fn test_ceiling_getter() {
        let mut output = OutputStage::new();
        assert!(approx_eq(output.ceiling(), 1.0, EPSILON));

        output.set_ceiling(0.8);
        assert!(approx_eq(output.ceiling(), 0.8, EPSILON));
    }

    #[test]
    fn test_volume_linear_getter() {
        let mut output = OutputStage::new();
        assert!(approx_eq(output.volume_linear(), 1.0, EPSILON));

        output.set_volume_db(-12.0);
        // -12dB ~= 0.251
        assert!(approx_eq(output.volume_linear(), 0.251, 0.001));
    }
}
