//! DSP Math Utilities for Guitar Amp Emulation
//!
//! This module provides fundamental mathematical operations used throughout
//! the DSP signal chain. All functions are `no_std` compatible and optimized
//! for real-time audio processing on embedded platforms.
//!
//! # Functions
//!
//! - [`tanh_approx`] - Fast hyperbolic tangent approximation for waveshaping
//! - [`db_to_linear`] - Convert decibels to linear gain
//! - [`linear_to_db`] - Convert linear gain to decibels
//! - [`soft_clip`] - Soft clipper for output limiting
//! - [`copysign_f32`] - Copy sign between floats
//! - [`clamp_f32`] - Clamp value to range
//! - [`lerp`] - Linear interpolation
//! - [`one_pole_coeff`] - Calculate one-pole filter coefficient
//!
//! # Usage
//!
//! These utilities are used in:
//! - **Preamp stages**: `tanh_approx` for tube saturation modeling
//! - **Output stage**: `soft_clip` for digital over prevention
//! - **All modules**: dB conversions for gain control

use libm::{expf, fabsf, log10f, powf};

/// Fast approximation of hyperbolic tangent for waveshaping.
///
/// This rational approximation provides good accuracy for typical audio
/// signal ranges while being computationally efficient for real-time use.
/// The approximation is derived from a Padé approximant.
///
/// # Arguments
///
/// * `x` - Input value (typically in range -3.0 to 3.0 for audio)
///
/// # Returns
///
/// Approximated tanh(x), bounded roughly to (-1.0, 1.0)
///
/// # Accuracy
///
/// Maximum error is approximately 0.02 compared to true tanh for |x| < 2.0.
/// The approximation saturates faster than true tanh at higher values.
///
/// # Example
///
/// ```
/// use guitar_amp_dsp::dsp_math::tanh_approx;
/// let saturated = tanh_approx(2.0);
/// assert!((saturated - 0.984).abs() < 0.01);
/// ```
#[inline]
#[must_use]
pub fn tanh_approx(x: f32) -> f32 {
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

/// Convert decibels to linear gain.
///
/// Implements the standard formula: linear = 10^(dB/20)
///
/// # Arguments
///
/// * `db` - Gain value in decibels
///
/// # Returns
///
/// Linear gain multiplier
///
/// # Example
///
/// ```
/// use guitar_amp_dsp::dsp_math::db_to_linear;
/// let gain = db_to_linear(-6.0);  // ~0.501
/// let unity = db_to_linear(0.0);  // 1.0
/// let boost = db_to_linear(20.0); // 10.0
/// ```
#[inline]
#[must_use]
pub fn db_to_linear(db: f32) -> f32 {
    powf(10.0, db / 20.0)
}

/// Convert linear gain to decibels.
///
/// Implements the standard formula: dB = 20 * log10(linear)
///
/// # Arguments
///
/// * `linear` - Linear gain multiplier (must be positive)
///
/// # Returns
///
/// Gain value in decibels. Returns negative infinity for linear <= 0.0
///
/// # Example
///
/// ```
/// use guitar_amp_dsp::dsp_math::linear_to_db;
/// let db = linear_to_db(0.5);   // ~-6.02 dB
/// let unity = linear_to_db(1.0); // 0.0 dB
/// let boost = linear_to_db(10.0); // 20.0 dB
/// ```
#[inline]
#[must_use]
pub fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        return f32::NEG_INFINITY;
    }
    20.0 * log10f(linear)
}

/// Soft clipper for output limiting.
///
/// Implements a soft knee compression algorithm that smoothly limits
/// signals exceeding the threshold while preserving signal integrity
/// below the threshold. This prevents harsh digital clipping artifacts.
///
/// From specification section 3.7 (Output Stage).
///
/// # Arguments
///
/// * `x` - Input sample value
/// * `ceiling` - Maximum absolute output level
///
/// # Returns
///
/// Soft-clipped output sample
///
/// # Algorithm
///
/// Below threshold (80% of ceiling): signal passes unchanged.
/// Above threshold: excess is compressed using the formula:
/// `compressed = threshold + excess / (1 + excess / (ceiling - threshold))`
///
/// # Example
///
/// ```
/// use guitar_amp_dsp::dsp_math::soft_clip;
/// let clipped = soft_clip(1.5, 1.0);
/// assert!(clipped < 1.0);
/// assert!(clipped > 0.8);
/// ```
#[inline]
#[must_use]
pub fn soft_clip(x: f32, ceiling: f32) -> f32 {
    let threshold = ceiling * 0.8;
    let abs_x = fabsf(x);
    if abs_x > threshold {
        let excess = abs_x - threshold;
        let headroom = ceiling - threshold;
        let compressed = threshold + excess / (1.0 + excess / headroom);
        copysign_f32(compressed, x)
    } else {
        x
    }
}

/// Copy sign from one float to another.
///
/// Returns a value with the magnitude of `magnitude` and the sign of `sign`.
/// This function is provided for `no_std` compatibility where the standard
/// library's `f32::copysign` may not be available.
///
/// # Arguments
///
/// * `magnitude` - The value whose absolute value will be used
/// * `sign` - The value whose sign will be used
///
/// # Returns
///
/// A value with |magnitude| and sign of `sign`
///
/// # Example
///
/// ```
/// use guitar_amp_dsp::dsp_math::copysign_f32;
/// assert_eq!(copysign_f32(3.0, -1.0), -3.0);
/// assert_eq!(copysign_f32(-3.0, 1.0), 3.0);
/// ```
#[inline]
#[must_use]
pub fn copysign_f32(magnitude: f32, sign: f32) -> f32 {
    let abs_mag = fabsf(magnitude);
    if sign < 0.0 {
        -abs_mag
    } else {
        abs_mag
    }
}

/// Clamp a value to a specified range.
///
/// Returns `x` constrained to the inclusive range [min, max].
/// This function is provided for `no_std` compatibility.
///
/// # Arguments
///
/// * `x` - Value to clamp
/// * `min` - Minimum bound (inclusive)
/// * `max` - Maximum bound (inclusive)
///
/// # Returns
///
/// Clamped value: min if x < min, max if x > max, otherwise x
///
/// # Example
///
/// ```
/// use guitar_amp_dsp::dsp_math::clamp_f32;
/// assert_eq!(clamp_f32(0.5, 0.0, 1.0), 0.5);
/// assert_eq!(clamp_f32(-0.5, 0.0, 1.0), 0.0);
/// assert_eq!(clamp_f32(1.5, 0.0, 1.0), 1.0);
/// ```
#[inline]
#[must_use]
pub fn clamp_f32(x: f32, min: f32, max: f32) -> f32 {
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}

/// Linear interpolation between two values.
///
/// Calculates the value at position `t` along the line from `a` to `b`.
///
/// # Arguments
///
/// * `a` - Start value (returned when t = 0)
/// * `b` - End value (returned when t = 1)
/// * `t` - Interpolation factor (typically 0.0 to 1.0)
///
/// # Returns
///
/// Interpolated value: `a + t * (b - a)`
///
/// # Note
///
/// Values of `t` outside [0, 1] will extrapolate beyond [a, b].
///
/// # Example
///
/// ```
/// use guitar_amp_dsp::dsp_math::lerp;
/// assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);
/// assert_eq!(lerp(0.0, 10.0, 0.0), 0.0);
/// assert_eq!(lerp(0.0, 10.0, 1.0), 10.0);
/// ```
#[inline]
#[must_use]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

/// Calculate coefficient for a one-pole low-pass filter.
///
/// Computes the feedback coefficient for a simple IIR filter:
/// `y[n] = (1 - coeff) * x[n] + coeff * y[n-1]`
///
/// This filter is commonly used for parameter smoothing and
/// simple frequency-dependent operations.
///
/// # Arguments
///
/// * `fc` - Cutoff frequency in Hz
/// * `sample_rate` - Sample rate in Hz
///
/// # Returns
///
/// Filter coefficient (typically 0.0 to 1.0)
///
/// # Formula
///
/// `coeff = exp(-2 * PI * fc / sample_rate)`
///
/// # Example
///
/// ```
/// use guitar_amp_dsp::dsp_math::one_pole_coeff;
/// let coeff = one_pole_coeff(1000.0, 48000.0);
/// // Higher cutoff -> smaller coefficient -> faster response
/// ```
#[inline]
#[must_use]
pub fn one_pole_coeff(fc: f32, sample_rate: f32) -> f32 {
    expf(-2.0 * core::f32::consts::PI * fc / sample_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32, epsilon: f32) -> bool {
        (a - b).abs() < epsilon
    }

    #[test]
    fn test_tanh_approx_zero() {
        assert!(approx_eq(tanh_approx(0.0), 0.0, EPSILON));
    }

    #[test]
    fn test_tanh_approx_positive() {
        // This Padé approximant gives tanh_approx(1.0) ~= 0.778
        // Real tanh(1.0) = 0.7615941559557649
        // Error is within expected bounds for this fast approximation
        let result = tanh_approx(1.0);
        assert!(approx_eq(result, 0.7777778, 0.001));
    }

    #[test]
    fn test_tanh_approx_negative() {
        // tanh is odd function: tanh(-x) = -tanh(x)
        let pos = tanh_approx(1.5);
        let neg = tanh_approx(-1.5);
        assert!(approx_eq(pos, -neg, EPSILON));
    }

    #[test]
    fn test_tanh_approx_saturation() {
        // At x=3, this approximation reaches 1.0 (the asymptote)
        // Real tanh(3.0) ~= 0.9951
        // The approximation saturates faster, which is acceptable for waveshaping
        let result = tanh_approx(3.0);
        assert!(approx_eq(result, 1.0, 0.001));
    }

    #[test]
    fn test_db_to_linear_zero() {
        // 0 dB = unity gain
        assert!(approx_eq(db_to_linear(0.0), 1.0, EPSILON));
    }

    #[test]
    fn test_db_to_linear_minus_6() {
        // -6 dB ~= 0.501187
        let result = db_to_linear(-6.0);
        assert!(approx_eq(result, 0.501187, 0.001));
    }

    #[test]
    fn test_db_to_linear_plus_20() {
        // +20 dB = 10.0
        assert!(approx_eq(db_to_linear(20.0), 10.0, EPSILON));
    }

    #[test]
    fn test_db_to_linear_minus_20() {
        // -20 dB = 0.1
        assert!(approx_eq(db_to_linear(-20.0), 0.1, EPSILON));
    }

    #[test]
    fn test_linear_to_db_unity() {
        // Unity gain = 0 dB
        assert!(approx_eq(linear_to_db(1.0), 0.0, EPSILON));
    }

    #[test]
    fn test_linear_to_db_half() {
        // 0.5 linear ~= -6.02 dB
        let result = linear_to_db(0.5);
        assert!(approx_eq(result, -6.0206, 0.001));
    }

    #[test]
    fn test_linear_to_db_ten() {
        // 10.0 linear = 20 dB
        assert!(approx_eq(linear_to_db(10.0), 20.0, EPSILON));
    }

    #[test]
    fn test_linear_to_db_zero() {
        // 0.0 linear = -infinity dB
        assert!(linear_to_db(0.0).is_infinite());
        assert!(linear_to_db(0.0) < 0.0);
    }

    #[test]
    fn test_linear_to_db_negative() {
        // Negative linear = -infinity dB
        assert!(linear_to_db(-1.0).is_infinite());
    }

    #[test]
    fn test_db_linear_roundtrip() {
        // Roundtrip: db -> linear -> db
        let original_db = -12.5;
        let linear = db_to_linear(original_db);
        let result_db = linear_to_db(linear);
        assert!(approx_eq(result_db, original_db, 0.001));
    }

    #[test]
    fn test_soft_clip_below_threshold() {
        // Below 80% threshold, signal passes unchanged
        let ceiling = 1.0;
        let threshold = ceiling * 0.8;
        let input = threshold * 0.5; // Well below threshold
        assert!(approx_eq(soft_clip(input, ceiling), input, EPSILON));
    }

    #[test]
    fn test_soft_clip_at_threshold() {
        // At threshold, minimal compression
        let ceiling = 1.0;
        let threshold = ceiling * 0.8;
        let result = soft_clip(threshold, ceiling);
        assert!(approx_eq(result, threshold, 0.001));
    }

    #[test]
    fn test_soft_clip_above_ceiling() {
        // Way above ceiling, output approaches but doesn't exceed ceiling
        let ceiling = 1.0;
        let input = 5.0;
        let result = soft_clip(input, ceiling);
        assert!(result < ceiling);
        assert!(result > ceiling * 0.8);
    }

    #[test]
    fn test_soft_clip_negative() {
        // Negative signals handled symmetrically
        let ceiling = 1.0;
        let input = -2.0;
        let result = soft_clip(input, ceiling);
        assert!(result > -ceiling);
        assert!(result < 0.0);
    }

    #[test]
    fn test_soft_clip_preserves_sign() {
        let ceiling = 1.0;
        assert!(soft_clip(2.0, ceiling) > 0.0);
        assert!(soft_clip(-2.0, ceiling) < 0.0);
    }

    #[test]
    fn test_copysign_positive_to_negative() {
        assert!(approx_eq(copysign_f32(3.0, -1.0), -3.0, EPSILON));
    }

    #[test]
    fn test_copysign_negative_to_positive() {
        assert!(approx_eq(copysign_f32(-3.0, 1.0), 3.0, EPSILON));
    }

    #[test]
    fn test_copysign_positive_to_positive() {
        assert!(approx_eq(copysign_f32(3.0, 1.0), 3.0, EPSILON));
    }

    #[test]
    fn test_copysign_negative_to_negative() {
        assert!(approx_eq(copysign_f32(-3.0, -1.0), -3.0, EPSILON));
    }

    #[test]
    fn test_clamp_within_range() {
        assert!(approx_eq(clamp_f32(0.5, 0.0, 1.0), 0.5, EPSILON));
    }

    #[test]
    fn test_clamp_below_min() {
        assert!(approx_eq(clamp_f32(-0.5, 0.0, 1.0), 0.0, EPSILON));
    }

    #[test]
    fn test_clamp_above_max() {
        assert!(approx_eq(clamp_f32(1.5, 0.0, 1.0), 1.0, EPSILON));
    }

    #[test]
    fn test_clamp_at_boundaries() {
        assert!(approx_eq(clamp_f32(0.0, 0.0, 1.0), 0.0, EPSILON));
        assert!(approx_eq(clamp_f32(1.0, 0.0, 1.0), 1.0, EPSILON));
    }

    #[test]
    fn test_lerp_zero() {
        assert!(approx_eq(lerp(0.0, 10.0, 0.0), 0.0, EPSILON));
    }

    #[test]
    fn test_lerp_one() {
        assert!(approx_eq(lerp(0.0, 10.0, 1.0), 10.0, EPSILON));
    }

    #[test]
    fn test_lerp_half() {
        assert!(approx_eq(lerp(0.0, 10.0, 0.5), 5.0, EPSILON));
    }

    #[test]
    fn test_lerp_negative_range() {
        assert!(approx_eq(lerp(-10.0, 10.0, 0.5), 0.0, EPSILON));
    }

    #[test]
    fn test_lerp_extrapolation() {
        // t > 1 extrapolates beyond b
        assert!(approx_eq(lerp(0.0, 10.0, 1.5), 15.0, EPSILON));
        // t < 0 extrapolates before a
        assert!(approx_eq(lerp(0.0, 10.0, -0.5), -5.0, EPSILON));
    }

    #[test]
    fn test_one_pole_coeff_low_cutoff() {
        // Low cutoff = high coefficient (slow filter)
        let coeff = one_pole_coeff(10.0, 48000.0);
        assert!(coeff > 0.99);
    }

    #[test]
    fn test_one_pole_coeff_high_cutoff() {
        // High cutoff = lower coefficient (fast filter)
        let coeff = one_pole_coeff(10000.0, 48000.0);
        assert!(coeff < 0.5);
    }

    #[test]
    fn test_one_pole_coeff_reference() {
        // Reference calculation: exp(-2 * PI * 1000 / 48000)
        // = exp(-0.1309) ~= 0.8773
        let coeff = one_pole_coeff(1000.0, 48000.0);
        assert!(approx_eq(coeff, 0.8773, 0.001));
    }

    #[test]
    fn test_one_pole_coeff_range() {
        // Coefficient should always be between 0 and 1
        let coeff_low = one_pole_coeff(1.0, 48000.0);
        let coeff_high = one_pole_coeff(20000.0, 48000.0);
        assert!(coeff_low > 0.0 && coeff_low < 1.0);
        assert!(coeff_high > 0.0 && coeff_high < 1.0);
    }
}
