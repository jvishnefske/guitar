//! Input Filter Module
//!
//! Models guitar pickup and cable resonance using a resonant low-pass filter.
//! This shapes the frequency response before the preamp, affecting the "voice"
//! of the guitar.
//!
//! # Overview
//!
//! Guitar pickups exhibit a characteristic resonant peak determined by the
//! inductance of the coil and the capacitance of the cable. This resonance
//! significantly affects the perceived tone:
//!
//! - **Single-coil pickups**: Higher resonant frequency (~4-5 kHz), brighter tone
//! - **Humbucker pickups**: Lower resonant frequency (~2-3 kHz), warmer tone
//! - **P90 pickups**: Middle ground (~3-4 kHz), balanced tone
//!
//! # Usage
//!
//! ```ignore
//! use crate::input_filter::{InputFilter, PickupParams};
//!
//! // Create filter with default parameters (3500 Hz, Q=1.0)
//! let mut filter = InputFilter::new(48000.0);
//!
//! // Configure for single-coil pickup
//! filter.set_params(PickupParams {
//!     freq_hz: 4500.0,
//!     q: 1.5,
//! });
//!
//! // Process audio
//! let output = filter.process_sample(input);
//! ```
//!
//! # Design Requirements (E2)
//!
//! - E2.1: 2nd order resonant low-pass filter (biquad)
//! - E2.2: Resonant frequency: 2-5 kHz configurable
//! - E2.3: Q factor: 0.5-2.0 configurable

use crate::biquad::Biquad;

/// Minimum allowed resonant frequency in Hz.
const MIN_FREQ_HZ: f32 = 2000.0;

/// Maximum allowed resonant frequency in Hz.
const MAX_FREQ_HZ: f32 = 5000.0;

/// Minimum allowed Q factor.
const MIN_Q: f32 = 0.5;

/// Maximum allowed Q factor.
const MAX_Q: f32 = 2.0;

/// Default resonant frequency in Hz (between single-coil and humbucker).
const DEFAULT_FREQ_HZ: f32 = 3500.0;

/// Default Q factor.
const DEFAULT_Q: f32 = 1.0;

/// Pickup resonance filter parameters.
///
/// These parameters model the electrical characteristics of guitar pickups
/// and cables. The resonant frequency and Q factor together determine the
/// "voice" of the pickup.
///
/// # Parameter Ranges
///
/// - `freq_hz`: 2000-5000 Hz (clamped if outside range)
/// - `q`: 0.5-2.0 (clamped if outside range)
///
/// # Typical Values
///
/// | Pickup Type  | Frequency | Q   | Character        |
/// |--------------|-----------|-----|------------------|
/// | Single-coil  | 4500 Hz   | 1.5 | Bright, chimey   |
/// | Humbucker    | 2500 Hz   | 1.0 | Warm, smooth     |
/// | P90          | 3500 Hz   | 1.2 | Balanced         |
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PickupParams {
    /// Resonant frequency in Hz (2000-5000).
    ///
    /// This corresponds to the LC resonance of the pickup inductance and
    /// cable capacitance. Lower values give a warmer tone, higher values
    /// give a brighter tone.
    pub freq_hz: f32,

    /// Quality factor (0.5-2.0).
    ///
    /// Controls the height and sharpness of the resonant peak.
    /// - Q < 0.707: No resonant peak, gentle rolloff
    /// - Q = 0.707: Butterworth (maximally flat)
    /// - Q > 0.707: Resonant peak at cutoff frequency
    /// - Q = 2.0: Pronounced resonant peak (very bright)
    pub q: f32,
}

impl Default for PickupParams {
    /// Returns default pickup parameters.
    ///
    /// Default values represent a middle-ground pickup character:
    /// - Frequency: 3500 Hz (between single-coil and humbucker)
    /// - Q: 1.0 (mild resonance)
    fn default() -> Self {
        Self {
            freq_hz: DEFAULT_FREQ_HZ,
            q: DEFAULT_Q,
        }
    }
}

impl PickupParams {
    /// Creates new pickup parameters with values clamped to valid ranges.
    ///
    /// # Arguments
    ///
    /// * `freq_hz` - Resonant frequency in Hz (will be clamped to 2000-5000)
    /// * `q` - Quality factor (will be clamped to 0.5-2.0)
    ///
    /// # Returns
    ///
    /// A `PickupParams` with clamped values.
    #[must_use]
    pub fn new(freq_hz: f32, q: f32) -> Self {
        Self {
            freq_hz: freq_hz.clamp(MIN_FREQ_HZ, MAX_FREQ_HZ),
            q: q.clamp(MIN_Q, MAX_Q),
        }
    }

    /// Creates parameters for a single-coil pickup character.
    ///
    /// Single-coil pickups have higher resonant frequencies due to their
    /// lower inductance, resulting in a brighter, more articulate tone.
    #[must_use]
    pub const fn single_coil() -> Self {
        Self {
            freq_hz: 4500.0,
            q: 1.5,
        }
    }

    /// Creates parameters for a humbucker pickup character.
    ///
    /// Humbuckers have lower resonant frequencies due to their higher
    /// inductance, resulting in a warmer, smoother tone.
    #[must_use]
    pub const fn humbucker() -> Self {
        Self {
            freq_hz: 2500.0,
            q: 1.0,
        }
    }

    /// Creates parameters for a P90 pickup character.
    ///
    /// P90s are a middle ground between single-coils and humbuckers,
    /// with moderate resonant frequency and Q.
    #[must_use]
    pub const fn p90() -> Self {
        Self {
            freq_hz: 3500.0,
            q: 1.2,
        }
    }
}

/// Input filter processor for pickup resonance modeling.
///
/// This filter models the frequency-dependent characteristics of guitar
/// pickups and cables. It implements a 2nd order resonant low-pass filter
/// using a biquad structure.
///
/// # Signal Flow
///
/// ```text
/// Guitar Signal --> [InputFilter] --> Preamp
///                    |
///                    v
///              Resonant LPF
///              (2-5 kHz, Q=0.5-2.0)
/// ```
///
/// # Thread Safety
///
/// This struct is not thread-safe. For concurrent access, wrap in
/// appropriate synchronization primitives.
pub struct InputFilter {
    /// The underlying biquad filter.
    filter: Biquad,
    /// Sample rate in Hz (cached for parameter updates).
    sample_rate: f32,
    /// Current filter parameters.
    params: PickupParams,
}

impl InputFilter {
    /// Creates a new input filter with default parameters.
    ///
    /// The filter is initialized with:
    /// - Frequency: 3500 Hz
    /// - Q: 1.0
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz (typically 48000)
    ///
    /// # Returns
    ///
    /// A new `InputFilter` ready for processing.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut filter = InputFilter::new(48000.0);
    /// ```
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        let params = PickupParams::default();
        Self {
            filter: Biquad::low_pass(params.freq_hz, params.q, sample_rate),
            sample_rate,
            params,
        }
    }

    /// Creates a new input filter with specified parameters.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz
    /// * `params` - Initial pickup parameters (will be clamped to valid ranges)
    ///
    /// # Returns
    ///
    /// A new `InputFilter` with the specified parameters.
    #[must_use]
    pub fn with_params(sample_rate: f32, params: PickupParams) -> Self {
        let clamped_params = PickupParams::new(params.freq_hz, params.q);
        Self {
            filter: Biquad::low_pass(clamped_params.freq_hz, clamped_params.q, sample_rate),
            sample_rate,
            params: clamped_params,
        }
    }

    /// Updates all filter parameters.
    ///
    /// Parameters are clamped to valid ranges:
    /// - Frequency: 2000-5000 Hz
    /// - Q: 0.5-2.0
    ///
    /// # Arguments
    ///
    /// * `params` - New pickup parameters
    ///
    /// # Note
    ///
    /// This recalculates the filter coefficients but does not reset the
    /// filter state. For smooth parameter changes during playback, this
    /// is generally desirable. Call [`reset`](Self::reset) if you need
    /// to clear the filter state.
    pub fn set_params(&mut self, params: PickupParams) {
        let freq = params.freq_hz.clamp(MIN_FREQ_HZ, MAX_FREQ_HZ);
        let q = params.q.clamp(MIN_Q, MAX_Q);
        self.params = PickupParams { freq_hz: freq, q };
        self.filter = Biquad::low_pass(freq, q, self.sample_rate);
    }

    /// Sets the resonant frequency.
    ///
    /// The frequency will be clamped to 2000-5000 Hz.
    ///
    /// # Arguments
    ///
    /// * `freq_hz` - New resonant frequency in Hz
    pub fn set_frequency(&mut self, freq_hz: f32) {
        let mut params = self.params;
        params.freq_hz = freq_hz;
        self.set_params(params);
    }

    /// Sets the Q factor.
    ///
    /// The Q factor will be clamped to 0.5-2.0.
    ///
    /// # Arguments
    ///
    /// * `q` - New Q factor
    pub fn set_q(&mut self, q: f32) {
        let mut params = self.params;
        params.q = q;
        self.set_params(params);
    }

    /// Processes a single audio sample through the filter.
    ///
    /// # Arguments
    ///
    /// * `x` - Input sample
    ///
    /// # Returns
    ///
    /// Filtered output sample.
    ///
    /// # Performance
    ///
    /// This function is designed for the audio hot path and is marked
    /// `#[inline]` to encourage inlining.
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        self.filter.process_sample(x)
    }

    /// Processes a buffer of audio samples in-place.
    ///
    /// This is more efficient than calling `process_sample` in a loop
    /// for large buffers.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Mutable slice of audio samples to process in-place
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        self.filter.process_buffer(buffer);
    }

    /// Resets the filter state to zero.
    ///
    /// Call this when:
    /// - Switching presets
    /// - Starting a new audio stream
    /// - After making large parameter changes to avoid transients
    pub fn reset(&mut self) {
        self.filter.reset();
    }

    /// Returns the current filter parameters.
    ///
    /// # Returns
    ///
    /// A copy of the current `PickupParams`.
    #[must_use]
    pub fn params(&self) -> PickupParams {
        self.params
    }

    /// Returns the sample rate.
    ///
    /// # Returns
    ///
    /// The sample rate in Hz that the filter was configured with.
    #[must_use]
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Computes the magnitude response at a given frequency.
    ///
    /// Useful for plotting frequency response or verifying filter behavior.
    ///
    /// # Arguments
    ///
    /// * `freq` - Frequency in Hz to evaluate
    ///
    /// # Returns
    ///
    /// Magnitude response (linear scale, not dB).
    #[must_use]
    pub fn magnitude_response(&self, freq: f32) -> f32 {
        self.filter.magnitude_response(freq, self.sample_rate)
    }

    /// Computes the magnitude response in decibels at a given frequency.
    ///
    /// # Arguments
    ///
    /// * `freq` - Frequency in Hz to evaluate
    ///
    /// # Returns
    ///
    /// Magnitude response in dB.
    #[must_use]
    pub fn magnitude_response_db(&self, freq: f32) -> f32 {
        self.filter.magnitude_response_db(freq, self.sample_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tolerance for floating-point comparisons.
    const EPSILON: f32 = 1e-5;

    /// Tolerance for frequency response tests.
    const RESPONSE_EPSILON: f32 = 0.05;

    /// Helper function to check if two floats are approximately equal.
    fn approx_eq(a: f32, b: f32, epsilon: f32) -> bool {
        (a - b).abs() < epsilon
    }

    // ===== Default Parameter Tests =====

    #[test]
    fn test_default_params_frequency() {
        let params = PickupParams::default();
        assert!(
            approx_eq(params.freq_hz, 3500.0, EPSILON),
            "Default frequency was {} expected 3500.0",
            params.freq_hz
        );
    }

    #[test]
    fn test_default_params_q() {
        let params = PickupParams::default();
        assert!(
            approx_eq(params.q, 1.0, EPSILON),
            "Default Q was {} expected 1.0",
            params.q
        );
    }

    #[test]
    fn test_filter_uses_default_params() {
        let filter = InputFilter::new(48000.0);
        let params = filter.params();

        assert!(
            approx_eq(params.freq_hz, 3500.0, EPSILON),
            "Filter frequency was {} expected 3500.0",
            params.freq_hz
        );
        assert!(
            approx_eq(params.q, 1.0, EPSILON),
            "Filter Q was {} expected 1.0",
            params.q
        );
    }

    // ===== Parameter Clamping Tests =====

    #[test]
    fn test_frequency_clamped_below_minimum() {
        let mut filter = InputFilter::new(48000.0);
        filter.set_frequency(1000.0); // Below 2000 Hz minimum

        let params = filter.params();
        assert!(
            approx_eq(params.freq_hz, 2000.0, EPSILON),
            "Frequency was {} expected 2000.0 (clamped)",
            params.freq_hz
        );
    }

    #[test]
    fn test_frequency_clamped_above_maximum() {
        let mut filter = InputFilter::new(48000.0);
        filter.set_frequency(8000.0); // Above 5000 Hz maximum

        let params = filter.params();
        assert!(
            approx_eq(params.freq_hz, 5000.0, EPSILON),
            "Frequency was {} expected 5000.0 (clamped)",
            params.freq_hz
        );
    }

    #[test]
    fn test_q_clamped_below_minimum() {
        let mut filter = InputFilter::new(48000.0);
        filter.set_q(0.1); // Below 0.5 minimum

        let params = filter.params();
        assert!(
            approx_eq(params.q, 0.5, EPSILON),
            "Q was {} expected 0.5 (clamped)",
            params.q
        );
    }

    #[test]
    fn test_q_clamped_above_maximum() {
        let mut filter = InputFilter::new(48000.0);
        filter.set_q(5.0); // Above 2.0 maximum

        let params = filter.params();
        assert!(
            approx_eq(params.q, 2.0, EPSILON),
            "Q was {} expected 2.0 (clamped)",
            params.q
        );
    }

    #[test]
    fn test_params_new_clamps_values() {
        let params = PickupParams::new(100.0, 10.0);

        assert!(
            approx_eq(params.freq_hz, 2000.0, EPSILON),
            "Frequency was {} expected 2000.0",
            params.freq_hz
        );
        assert!(
            approx_eq(params.q, 2.0, EPSILON),
            "Q was {} expected 2.0",
            params.q
        );
    }

    #[test]
    fn test_set_params_clamps_both() {
        let mut filter = InputFilter::new(48000.0);
        filter.set_params(PickupParams {
            freq_hz: 10000.0,
            q: 0.0,
        });

        let params = filter.params();
        assert!(
            approx_eq(params.freq_hz, 5000.0, EPSILON),
            "Frequency was {} expected 5000.0",
            params.freq_hz
        );
        assert!(
            approx_eq(params.q, 0.5, EPSILON),
            "Q was {} expected 0.5",
            params.q
        );
    }

    // ===== Frequency Response Tests =====

    #[test]
    fn test_dc_passes_through() {
        let filter = InputFilter::new(48000.0);

        // DC (very low frequency) should pass through a low-pass filter
        let dc_response = filter.magnitude_response(10.0);

        assert!(
            approx_eq(dc_response, 1.0, RESPONSE_EPSILON),
            "DC response was {} expected ~1.0",
            dc_response
        );
    }

    #[test]
    fn test_high_frequency_attenuated() {
        let filter = InputFilter::new(48000.0);

        // Well above the cutoff (3500 Hz), signal should be attenuated
        let hf_response = filter.magnitude_response(15000.0);

        assert!(
            hf_response < 0.2,
            "High frequency response was {} expected < 0.2",
            hf_response
        );
    }

    #[test]
    fn test_resonant_peak_with_high_q() {
        // Q > 0.707 should produce a resonant peak
        let mut filter = InputFilter::new(48000.0);
        filter.set_params(PickupParams {
            freq_hz: 3500.0,
            q: 2.0,
        });

        // At resonance with high Q, response should exceed unity
        let resonance_response = filter.magnitude_response(3500.0);

        assert!(
            resonance_response > 1.0,
            "Resonant peak was {} expected > 1.0",
            resonance_response
        );
    }

    #[test]
    fn test_no_peak_with_low_q() {
        // Q < 0.707 should not produce a peak above unity
        let mut filter = InputFilter::new(48000.0);
        filter.set_params(PickupParams {
            freq_hz: 3500.0,
            q: 0.5,
        });

        // With low Q, the cutoff response should be below unity
        let cutoff_response = filter.magnitude_response(3500.0);

        assert!(
            cutoff_response <= 1.0,
            "Response at cutoff was {} expected <= 1.0 (no peak)",
            cutoff_response
        );
    }

    #[test]
    fn test_frequency_affects_response() {
        let mut filter = InputFilter::new(48000.0);

        // Set low frequency (humbucker-like)
        filter.set_frequency(2000.0);
        let low_fc_response_at_4k = filter.magnitude_response(4000.0);

        // Set high frequency (single-coil-like)
        filter.set_frequency(5000.0);
        let high_fc_response_at_4k = filter.magnitude_response(4000.0);

        // Higher cutoff should pass more of the 4 kHz signal
        assert!(
            high_fc_response_at_4k > low_fc_response_at_4k,
            "Higher fc response ({}) should be greater than lower fc response ({})",
            high_fc_response_at_4k,
            low_fc_response_at_4k
        );
    }

    // ===== Processing Tests =====

    #[test]
    fn test_process_sample_changes_signal() {
        let mut filter = InputFilter::new(48000.0);

        // Feed in a high-frequency component (impulse has all frequencies)
        let _ = filter.process_sample(1.0);
        let output = filter.process_sample(0.0);

        // Output should be non-zero due to filter state
        assert!(
            output != 0.0,
            "Filter state should affect subsequent samples"
        );
    }

    #[test]
    fn test_process_buffer_matches_sample_by_sample() {
        let mut filter1 = InputFilter::new(48000.0);
        let mut filter2 = InputFilter::new(48000.0);

        let input = [0.5, -0.3, 0.8, -0.1, 0.0, 0.2, -0.4, 0.6];

        // Process sample-by-sample
        let mut output1 = input;
        for sample in output1.iter_mut() {
            *sample = filter1.process_sample(*sample);
        }

        // Process as buffer
        let mut output2 = input;
        filter2.process_buffer(&mut output2);

        // Results should match
        for (i, (a, b)) in output1.iter().zip(output2.iter()).enumerate() {
            assert!(
                approx_eq(*a, *b, EPSILON),
                "Mismatch at sample {}: {} vs {}",
                i,
                a,
                b
            );
        }
    }

    #[test]
    fn test_reset_clears_state() {
        let mut filter = InputFilter::new(48000.0);

        // Build up state
        for _ in 0..100 {
            filter.process_sample(1.0);
        }

        // Reset
        filter.reset();

        // After reset, processing zero should give zero
        let output = filter.process_sample(0.0);
        assert!(
            approx_eq(output, 0.0, EPSILON),
            "Output after reset was {} expected 0.0",
            output
        );
    }

    // ===== Preset Tests =====

    #[test]
    fn test_single_coil_preset() {
        let params = PickupParams::single_coil();

        assert!(
            approx_eq(params.freq_hz, 4500.0, EPSILON),
            "Single-coil frequency was {} expected 4500.0",
            params.freq_hz
        );
        assert!(
            approx_eq(params.q, 1.5, EPSILON),
            "Single-coil Q was {} expected 1.5",
            params.q
        );
    }

    #[test]
    fn test_humbucker_preset() {
        let params = PickupParams::humbucker();

        assert!(
            approx_eq(params.freq_hz, 2500.0, EPSILON),
            "Humbucker frequency was {} expected 2500.0",
            params.freq_hz
        );
        assert!(
            approx_eq(params.q, 1.0, EPSILON),
            "Humbucker Q was {} expected 1.0",
            params.q
        );
    }

    #[test]
    fn test_p90_preset() {
        let params = PickupParams::p90();

        assert!(
            approx_eq(params.freq_hz, 3500.0, EPSILON),
            "P90 frequency was {} expected 3500.0",
            params.freq_hz
        );
        assert!(
            approx_eq(params.q, 1.2, EPSILON),
            "P90 Q was {} expected 1.2",
            params.q
        );
    }

    // ===== Constructor Tests =====

    #[test]
    fn test_with_params_constructor() {
        let params = PickupParams::single_coil();
        let filter = InputFilter::with_params(48000.0, params);

        let actual = filter.params();
        assert!(
            approx_eq(actual.freq_hz, 4500.0, EPSILON),
            "Frequency was {} expected 4500.0",
            actual.freq_hz
        );
        assert!(
            approx_eq(actual.q, 1.5, EPSILON),
            "Q was {} expected 1.5",
            actual.q
        );
    }

    #[test]
    fn test_with_params_clamps() {
        let params = PickupParams {
            freq_hz: 10000.0,
            q: 5.0,
        };
        let filter = InputFilter::with_params(48000.0, params);

        let actual = filter.params();
        assert!(
            approx_eq(actual.freq_hz, 5000.0, EPSILON),
            "Frequency was {} expected 5000.0",
            actual.freq_hz
        );
        assert!(
            approx_eq(actual.q, 2.0, EPSILON),
            "Q was {} expected 2.0",
            actual.q
        );
    }

    #[test]
    fn test_sample_rate_stored() {
        let filter = InputFilter::new(44100.0);
        assert!(
            approx_eq(filter.sample_rate(), 44100.0, EPSILON),
            "Sample rate was {} expected 44100.0",
            filter.sample_rate()
        );
    }

    // ===== Numerical Stability Tests =====

    #[test]
    fn test_numerical_stability_long_signal() {
        let mut filter = InputFilter::new(48000.0);

        // Process many samples without NaN/Inf
        for i in 0..100000 {
            let input = if i % 2 == 0 { 0.5 } else { -0.5 };
            let output = filter.process_sample(input);

            assert!(
                output.is_finite(),
                "Output became non-finite at sample {}",
                i
            );
        }
    }

    #[test]
    fn test_numerical_stability_extreme_params() {
        // Test with edge case parameters
        let mut filter = InputFilter::with_params(
            48000.0,
            PickupParams {
                freq_hz: 2000.0,
                q: 2.0,
            },
        );

        for i in 0..10000 {
            let input = libm::sinf(2.0 * core::f32::consts::PI * 1000.0 * i as f32 / 48000.0);
            let output = filter.process_sample(input);

            assert!(
                output.is_finite(),
                "Output became non-finite at sample {}",
                i
            );
            assert!(
                output.abs() < 10.0,
                "Output magnitude {} unreasonably large at sample {}",
                output.abs(),
                i
            );
        }
    }
}
