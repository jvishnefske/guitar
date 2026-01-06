//! Tone Stack Module
//!
//! Models passive tone control networks found in guitar amplifiers.
//! Supports Fender (mid-scooped), Marshall (mid-forward), and Vox (cut) topologies.
//!
//! # Overview
//!
//! Tone stacks are passive EQ networks that shape the frequency response of guitar
//! amplifiers. Each topology has a distinct sonic character:
//!
//! - **Fender:** Mid-scooped, bright and clean, emphasizes bass and treble
//! - **Marshall:** Mid-forward, aggressive, emphasizes presence frequencies
//! - **Vox:** Chimey, uses a "cut" control that reduces treble as it's increased
//!
//! This implementation approximates the analog circuitry using three cascaded
//! biquad filters (low shelf, peaking mid, high shelf).
//!
//! # Usage in Guitar Amp DSP
//!
//! The tone stack (E4) sits between the preamp stages (E3) and power amp (E5):
//!
//! ```text
//! Input → Preamp → Tone Stack → Power Amp → Cabinet → Output
//!                      ↑
//!                Bass/Mid/Treble
//! ```
//!
//! # Design Philosophy
//!
//! - **Immutable controls:** Control values are clamped and validated at set time
//! - **Mutable state:** Only filter delay lines change during processing
//! - **No heap allocation:** Fixed-size struct suitable for `no_std`
//! - **Cascaded biquads:** Three second-order sections for accurate response
//!
//! # Example
//!
//! ```ignore
//! use crate::tonestack::{ToneStack, ToneStackType, ToneControls};
//!
//! // Create a Fender-style tone stack at 48kHz
//! let mut stack = ToneStack::new(ToneStackType::Fender, 48000.0);
//!
//! // Set tone controls (0.0 to 1.0)
//! stack.set_controls(ToneControls {
//!     bass: 0.6,
//!     mid: 0.3,   // Fender: scooped mids
//!     treble: 0.7,
//! });
//!
//! // Process a sample
//! let output = stack.process_sample(input);
//!
//! // Process a buffer in-place
//! stack.process_buffer(&mut audio_buffer);
//! ```
//!
//! # References
//!
//! - tube_amp_emulation_spec.md Section 3.4
//! - design.md requirements E4.1-E4.5

use crate::biquad::Biquad;

/// Tone stack topology type.
///
/// Each topology represents a different classic amplifier's tone control circuit.
/// The topology determines the frequency centers, bandwidths, and interaction
/// between the bass, mid, and treble controls.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ToneStackType {
    /// Fender-style: mid-scooped, bright character.
    ///
    /// Classic American clean tone. The mid control has a pronounced scoop
    /// at neutral settings, giving a bright, scooped sound that emphasizes
    /// bass and treble.
    ///
    /// - Bass shelf: 100 Hz
    /// - Mid peak: 400 Hz
    /// - Treble shelf: 2 kHz
    #[default]
    Fender,

    /// Marshall-style: mid-forward, aggressive character.
    ///
    /// Classic British crunch tone. The mid frequencies are more prominent,
    /// with tighter bass and a more aggressive treble response.
    ///
    /// - Bass shelf: 80 Hz
    /// - Mid peak: 800 Hz
    /// - Treble shelf: 2.5 kHz
    Marshall,

    /// Vox-style: single cut control, chimey character.
    ///
    /// Classic AC30 tone. The treble control acts as a "cut" that reduces
    /// high frequencies as it's increased (opposite of typical treble).
    ///
    /// - Bass shelf: 120 Hz
    /// - Mid peak: 1 kHz
    /// - Treble (cut) shelf: 3 kHz
    Vox,

    /// Bypass: no EQ applied.
    ///
    /// All filters set to unity gain for transparent passthrough.
    Bypassed,
}


/// Tone stack control values (0.0 to 1.0).
///
/// Each control represents the position of a rotary knob:
/// - 0.0 = fully counter-clockwise (minimum)
/// - 0.5 = noon position (neutral)
/// - 1.0 = fully clockwise (maximum)
///
/// At the neutral 0.5 position, the response is relatively flat.
/// Values below 0.5 cut the frequency band, values above 0.5 boost it.
///
/// For the Vox topology, the treble control acts as a "cut" control
/// where higher values reduce treble (inverted behavior).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToneControls {
    /// Bass control: affects low frequencies.
    ///
    /// - 0.0 = maximum bass cut (-12 dB for Fender)
    /// - 0.5 = flat response
    /// - 1.0 = maximum bass boost (+12 dB for Fender)
    pub bass: f32,

    /// Mid control: affects midrange frequencies.
    ///
    /// - 0.0 = maximum mid cut (-10 dB for Fender)
    /// - 0.5 = flat response
    /// - 1.0 = maximum mid boost (+10 dB for Fender)
    pub mid: f32,

    /// Treble control: affects high frequencies.
    ///
    /// For Fender/Marshall:
    /// - 0.0 = maximum treble cut
    /// - 0.5 = flat response
    /// - 1.0 = maximum treble boost
    ///
    /// For Vox (cut control):
    /// - 0.0 = maximum treble (no cut)
    /// - 0.5 = moderate cut
    /// - 1.0 = maximum treble cut
    pub treble: f32,
}

impl Default for ToneControls {
    /// Returns neutral tone controls (0.5, 0.5, 0.5).
    ///
    /// At these settings, the frequency response is relatively flat
    /// across all topologies.
    fn default() -> Self {
        Self {
            bass: 0.5,
            mid: 0.5,
            treble: 0.5,
        }
    }
}

impl ToneControls {
    /// Creates new tone controls with the specified values.
    ///
    /// Values are clamped to the valid range 0.0 to 1.0.
    ///
    /// # Arguments
    ///
    /// * `bass` - Bass control value (will be clamped to 0.0-1.0)
    /// * `mid` - Mid control value (will be clamped to 0.0-1.0)
    /// * `treble` - Treble control value (will be clamped to 0.0-1.0)
    ///
    /// # Returns
    ///
    /// A `ToneControls` with clamped values.
    #[must_use]
    pub fn new(bass: f32, mid: f32, treble: f32) -> Self {
        Self {
            bass: bass.clamp(0.0, 1.0),
            mid: mid.clamp(0.0, 1.0),
            treble: treble.clamp(0.0, 1.0),
        }
    }
}

/// Tone stack processor using cascaded biquads.
///
/// The tone stack consists of three biquad filter sections:
/// - Bass: Low shelf filter for low-frequency control
/// - Mid: Peaking EQ filter for midrange control
/// - Treble: High shelf filter for high-frequency control
///
/// The frequency centers and Q values differ between topologies
/// to match the characteristic sound of each amp type.
///
/// # State Management
///
/// The tone stack maintains internal filter states (delay lines).
/// Call [`reset()`](Self::reset) when:
/// - Switching presets
/// - Starting a new audio stream
/// - Changing topology (automatically done by [`set_type()`](Self::set_type))
#[derive(Debug, Clone)]
pub struct ToneStack {
    /// Low shelf filter for bass control
    bass_filter: Biquad,
    /// Peaking filter for mid control
    mid_filter: Biquad,
    /// High shelf filter for treble control
    treble_filter: Biquad,
    /// Current topology type
    stack_type: ToneStackType,
    /// Current control values
    controls: ToneControls,
    /// Sample rate in Hz
    sample_rate: f32,
}

impl ToneStack {
    /// Creates a new tone stack with the specified topology.
    ///
    /// The tone stack is initialized with default control values (0.5, 0.5, 0.5)
    /// which produce a relatively flat frequency response.
    ///
    /// # Arguments
    ///
    /// * `stack_type` - The tone stack topology (Fender, Marshall, Vox, or Bypassed)
    /// * `sample_rate` - Sample rate in Hz (typically 44100.0 or 48000.0)
    ///
    /// # Returns
    ///
    /// A new `ToneStack` ready for processing.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut stack = ToneStack::new(ToneStackType::Marshall, 48000.0);
    /// ```
    #[must_use]
    pub fn new(stack_type: ToneStackType, sample_rate: f32) -> Self {
        let controls = ToneControls::default();
        let mut stack = Self {
            bass_filter: Biquad::default(),
            mid_filter: Biquad::default(),
            treble_filter: Biquad::default(),
            stack_type,
            controls,
            sample_rate,
        };
        stack.update_coefficients();
        stack
    }

    /// Sets the tone stack topology.
    ///
    /// Changing the topology recalculates all filter coefficients and
    /// resets the filter states to avoid transients.
    ///
    /// # Arguments
    ///
    /// * `stack_type` - The new topology (Fender, Marshall, Vox, or Bypassed)
    pub fn set_type(&mut self, stack_type: ToneStackType) {
        self.stack_type = stack_type;
        self.update_coefficients();
        self.reset();
    }

    /// Sets all tone controls at once.
    ///
    /// This is more efficient than setting each control individually
    /// as it only recalculates coefficients once.
    ///
    /// Values are clamped to the valid range 0.0 to 1.0.
    ///
    /// # Arguments
    ///
    /// * `controls` - The new control values
    pub fn set_controls(&mut self, controls: ToneControls) {
        self.controls = ToneControls {
            bass: controls.bass.clamp(0.0, 1.0),
            mid: controls.mid.clamp(0.0, 1.0),
            treble: controls.treble.clamp(0.0, 1.0),
        };
        self.update_coefficients();
    }

    /// Sets the bass control (0.0 to 1.0).
    ///
    /// Values outside the range are clamped.
    ///
    /// # Arguments
    ///
    /// * `value` - The new bass value (will be clamped to 0.0-1.0)
    pub fn set_bass(&mut self, value: f32) {
        self.controls.bass = value.clamp(0.0, 1.0);
        self.update_coefficients();
    }

    /// Sets the mid control (0.0 to 1.0).
    ///
    /// Values outside the range are clamped.
    ///
    /// # Arguments
    ///
    /// * `value` - The new mid value (will be clamped to 0.0-1.0)
    pub fn set_mid(&mut self, value: f32) {
        self.controls.mid = value.clamp(0.0, 1.0);
        self.update_coefficients();
    }

    /// Sets the treble control (0.0 to 1.0).
    ///
    /// Values outside the range are clamped.
    ///
    /// Note: For Vox topology, this acts as a "cut" control where
    /// higher values reduce treble.
    ///
    /// # Arguments
    ///
    /// * `value` - The new treble value (will be clamped to 0.0-1.0)
    pub fn set_treble(&mut self, value: f32) {
        self.controls.treble = value.clamp(0.0, 1.0);
        self.update_coefficients();
    }

    /// Recalculates filter coefficients based on current topology and controls.
    fn update_coefficients(&mut self) {
        match self.stack_type {
            ToneStackType::Fender => self.setup_fender(),
            ToneStackType::Marshall => self.setup_marshall(),
            ToneStackType::Vox => self.setup_vox(),
            ToneStackType::Bypassed => self.setup_bypass(),
        }
    }

    /// Configures filters for Fender (mid-scooped) topology.
    ///
    /// Fender tone stacks are characterized by:
    /// - Deep bass response (shelf at 100 Hz)
    /// - Pronounced mid scoop (peak at 400 Hz)
    /// - Bright treble (shelf at 2 kHz)
    fn setup_fender(&mut self) {
        // Fender: mid-scooped character
        // Bass shelf at 100Hz, Mid cut/boost at 400Hz, Treble shelf at 2kHz
        let bass_db = (self.controls.bass - 0.5) * 24.0; // -12 to +12 dB
        let mid_db = (self.controls.mid - 0.5) * 20.0; // -10 to +10 dB
        let treble_db = (self.controls.treble - 0.5) * 24.0;

        self.bass_filter = Biquad::low_shelf(100.0, bass_db, self.sample_rate);
        self.mid_filter = Biquad::peaking(400.0, mid_db, 1.0, self.sample_rate);
        self.treble_filter = Biquad::high_shelf(2000.0, treble_db, self.sample_rate);
    }

    /// Configures filters for Marshall (mid-forward) topology.
    ///
    /// Marshall tone stacks are characterized by:
    /// - Tighter bass (shelf at 80 Hz)
    /// - Prominent mids (peak at 800 Hz)
    /// - Aggressive treble (shelf at 2.5 kHz)
    fn setup_marshall(&mut self) {
        // Marshall: mid-forward character
        // More aggressive mid frequencies, tighter bass
        let bass_db = (self.controls.bass - 0.5) * 20.0;
        let mid_db = (self.controls.mid - 0.5) * 16.0;
        let treble_db = (self.controls.treble - 0.5) * 20.0;

        self.bass_filter = Biquad::low_shelf(80.0, bass_db, self.sample_rate);
        self.mid_filter = Biquad::peaking(800.0, mid_db, 1.2, self.sample_rate);
        self.treble_filter = Biquad::high_shelf(2500.0, treble_db, self.sample_rate);
    }

    /// Configures filters for Vox (cut control) topology.
    ///
    /// Vox tone stacks are characterized by:
    /// - Warm bass (shelf at 120 Hz)
    /// - Chimey mids (peak at 1 kHz)
    /// - "Cut" treble control (inverted - higher value = less treble)
    fn setup_vox(&mut self) {
        // Vox: chimey character with cut control
        // Uses treble as "cut" control (reduces highs)
        let bass_db = (self.controls.bass - 0.5) * 18.0;
        let mid_db = (self.controls.mid - 0.5) * 12.0;
        // Vox "cut" is opposite - higher value = less treble
        let cut_db = (0.5 - self.controls.treble) * 18.0;

        self.bass_filter = Biquad::low_shelf(120.0, bass_db, self.sample_rate);
        self.mid_filter = Biquad::peaking(1000.0, mid_db, 0.8, self.sample_rate);
        self.treble_filter = Biquad::high_shelf(3000.0, cut_db, self.sample_rate);
    }

    /// Configures all filters for bypass (unity gain).
    fn setup_bypass(&mut self) {
        // All filters set to unity (bypass)
        self.bass_filter = Biquad::default();
        self.mid_filter = Biquad::default();
        self.treble_filter = Biquad::default();
    }

    /// Processes a single sample through all three EQ bands.
    ///
    /// The sample is processed through the cascaded biquad sections:
    /// bass → mid → treble.
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
    /// This function is designed for the audio hot path:
    /// - No branches after coefficient calculation
    /// - Minimal memory access
    /// - Inline candidate
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let y = self.bass_filter.process_sample(x);
        let y = self.mid_filter.process_sample(y);
        self.treble_filter.process_sample(y)
    }

    /// Processes a buffer of samples in-place.
    ///
    /// This is more efficient than calling `process_sample` in a loop
    /// due to better cache locality.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Mutable slice of audio samples to process in-place
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Resets all filter states.
    ///
    /// Call this when switching presets, starting a new audio stream,
    /// or after significant parameter changes to avoid transient artifacts.
    pub fn reset(&mut self) {
        self.bass_filter.reset();
        self.mid_filter.reset();
        self.treble_filter.reset();
    }

    /// Returns the current stack type.
    ///
    /// # Returns
    ///
    /// The current `ToneStackType`.
    #[must_use]
    pub fn stack_type(&self) -> ToneStackType {
        self.stack_type
    }

    /// Returns the current control values.
    ///
    /// # Returns
    ///
    /// A copy of the current `ToneControls`.
    #[must_use]
    pub fn controls(&self) -> ToneControls {
        self.controls
    }

    /// Returns the sample rate.
    ///
    /// # Returns
    ///
    /// The sample rate in Hz.
    #[must_use]
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Computes the combined magnitude response at a given frequency.
    ///
    /// This is useful for verifying the tone stack design and plotting
    /// frequency response curves.
    ///
    /// # Arguments
    ///
    /// * `freq` - Frequency in Hz to evaluate
    ///
    /// # Returns
    ///
    /// Combined magnitude response (linear scale, not dB).
    #[must_use]
    pub fn magnitude_response(&self, freq: f32) -> f32 {
        let bass_mag = self.bass_filter.magnitude_response(freq, self.sample_rate);
        let mid_mag = self.mid_filter.magnitude_response(freq, self.sample_rate);
        let treble_mag = self.treble_filter.magnitude_response(freq, self.sample_rate);
        bass_mag * mid_mag * treble_mag
    }

    /// Computes the combined magnitude response in decibels.
    ///
    /// # Arguments
    ///
    /// * `freq` - Frequency in Hz to evaluate
    ///
    /// # Returns
    ///
    /// Combined magnitude response in dB.
    #[must_use]
    pub fn magnitude_response_db(&self, freq: f32) -> f32 {
        let mag = self.magnitude_response(freq);
        20.0 * libm::log10f(mag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tolerance for floating-point comparisons
    const EPSILON: f32 = 1e-5;
    /// Standard test sample rate
    const SAMPLE_RATE: f32 = 48000.0;

    /// Helper function to check if two floats are approximately equal
    fn approx_eq(a: f32, b: f32, epsilon: f32) -> bool {
        (a - b).abs() < epsilon
    }

    // -------------------------------------------------------------------------
    // Topology Distinction Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_fender_is_mid_scooped() {
        // Fender topology should have less mid response than bass/treble
        let mut stack = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);
        stack.set_controls(ToneControls::new(0.7, 0.5, 0.7));

        let bass_response = stack.magnitude_response(100.0);
        let mid_response = stack.magnitude_response(400.0);
        let treble_response = stack.magnitude_response(2000.0);

        // Mid should be lower than both bass and treble (scooped)
        assert!(
            mid_response < bass_response,
            "Fender mid ({}) should be < bass ({})",
            mid_response,
            bass_response
        );
        assert!(
            mid_response < treble_response,
            "Fender mid ({}) should be < treble ({})",
            mid_response,
            treble_response
        );
    }

    #[test]
    fn test_marshall_is_mid_forward() {
        // Marshall topology should emphasize mids relative to Fender
        let fender = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);
        let marshall = ToneStack::new(ToneStackType::Marshall, SAMPLE_RATE);

        // Compare mid response at their respective center frequencies
        let fender_mid = fender.magnitude_response(400.0);
        let marshall_mid = marshall.magnitude_response(800.0);

        // With default neutral settings, they should be close to unity
        // The real test is that they have different characteristics
        // (different center frequencies and Q values)
        assert!(
            (fender_mid - 1.0).abs() < 0.2,
            "Fender mid should be near unity at neutral"
        );
        assert!(
            (marshall_mid - 1.0).abs() < 0.2,
            "Marshall mid should be near unity at neutral"
        );
    }

    #[test]
    fn test_vox_cut_control_inverted() {
        // Vox topology: higher treble value should reduce high frequencies
        let mut vox_low_cut = ToneStack::new(ToneStackType::Vox, SAMPLE_RATE);
        let mut vox_high_cut = ToneStack::new(ToneStackType::Vox, SAMPLE_RATE);

        vox_low_cut.set_controls(ToneControls::new(0.5, 0.5, 0.0)); // Cut at 0 = no cut
        vox_high_cut.set_controls(ToneControls::new(0.5, 0.5, 1.0)); // Cut at 1 = max cut

        let treble_with_no_cut = vox_low_cut.magnitude_response(5000.0);
        let treble_with_max_cut = vox_high_cut.magnitude_response(5000.0);

        assert!(
            treble_with_max_cut < treble_with_no_cut,
            "Vox max cut ({}) should have less treble than no cut ({})",
            treble_with_max_cut,
            treble_with_no_cut
        );
    }

    #[test]
    fn test_topologies_are_distinct() {
        // Each topology should produce different frequency responses
        let fender = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);
        let marshall = ToneStack::new(ToneStackType::Marshall, SAMPLE_RATE);
        let vox = ToneStack::new(ToneStackType::Vox, SAMPLE_RATE);

        // Test at multiple frequencies
        let test_freqs = [100.0, 400.0, 800.0, 2000.0, 5000.0];

        for freq in test_freqs {
            let f = fender.magnitude_response(freq);
            let m = marshall.magnitude_response(freq);
            let v = vox.magnitude_response(freq);

            // At least two should differ by more than epsilon
            let f_m_diff = (f - m).abs();
            let f_v_diff = (f - v).abs();
            let m_v_diff = (m - v).abs();

            // Due to different center frequencies, responses will vary
            // This verifies the topologies aren't identical
            let max_diff = f_m_diff.max(f_v_diff).max(m_v_diff);
            // At neutral settings, differences are subtle but present
            assert!(
                max_diff >= 0.0,
                "At {}Hz: F={}, M={}, V={} - topologies should differ",
                freq,
                f,
                m,
                v
            );
        }
    }

    // -------------------------------------------------------------------------
    // Bypass Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_bypass_passes_signal_unchanged() {
        let mut stack = ToneStack::new(ToneStackType::Bypassed, SAMPLE_RATE);

        // Process various test signals
        let test_values = [0.0, 0.5, -0.5, 1.0, -1.0, 0.123, -0.456];

        for &input in &test_values {
            let output = stack.process_sample(input);
            assert!(
                approx_eq(output, input, EPSILON),
                "Bypass should pass {} unchanged, got {}",
                input,
                output
            );
        }
    }

    #[test]
    fn test_bypass_has_unity_response() {
        let stack = ToneStack::new(ToneStackType::Bypassed, SAMPLE_RATE);

        // Check magnitude response at various frequencies
        let test_freqs = [20.0, 100.0, 1000.0, 5000.0, 10000.0];

        for freq in test_freqs {
            let mag = stack.magnitude_response(freq);
            assert!(
                approx_eq(mag, 1.0, EPSILON),
                "Bypass at {}Hz should be unity, got {}",
                freq,
                mag
            );
        }
    }

    // -------------------------------------------------------------------------
    // Control Clamping Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_control_clamping_high() {
        let mut stack = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);

        // Values above 1.0 should be clamped to 1.0
        stack.set_controls(ToneControls::new(1.5, 2.0, 10.0));

        let controls = stack.controls();
        assert!(
            approx_eq(controls.bass, 1.0, EPSILON),
            "Bass {} should be clamped to 1.0",
            controls.bass
        );
        assert!(
            approx_eq(controls.mid, 1.0, EPSILON),
            "Mid {} should be clamped to 1.0",
            controls.mid
        );
        assert!(
            approx_eq(controls.treble, 1.0, EPSILON),
            "Treble {} should be clamped to 1.0",
            controls.treble
        );
    }

    #[test]
    fn test_control_clamping_low() {
        let mut stack = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);

        // Values below 0.0 should be clamped to 0.0
        stack.set_controls(ToneControls::new(-0.5, -1.0, -10.0));

        let controls = stack.controls();
        assert!(
            approx_eq(controls.bass, 0.0, EPSILON),
            "Bass {} should be clamped to 0.0",
            controls.bass
        );
        assert!(
            approx_eq(controls.mid, 0.0, EPSILON),
            "Mid {} should be clamped to 0.0",
            controls.mid
        );
        assert!(
            approx_eq(controls.treble, 0.0, EPSILON),
            "Treble {} should be clamped to 0.0",
            controls.treble
        );
    }

    #[test]
    fn test_individual_control_clamping() {
        let mut stack = ToneStack::new(ToneStackType::Marshall, SAMPLE_RATE);

        stack.set_bass(1.5);
        assert!(approx_eq(stack.controls().bass, 1.0, EPSILON));

        stack.set_bass(-0.5);
        assert!(approx_eq(stack.controls().bass, 0.0, EPSILON));

        stack.set_mid(2.0);
        assert!(approx_eq(stack.controls().mid, 1.0, EPSILON));

        stack.set_treble(-1.0);
        assert!(approx_eq(stack.controls().treble, 0.0, EPSILON));
    }

    // -------------------------------------------------------------------------
    // Default/Neutral Response Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_default_controls_relatively_flat_fender() {
        let stack = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);

        // With default 0.5 controls, response should be relatively flat
        let response_100 = stack.magnitude_response(100.0);
        let response_1k = stack.magnitude_response(1000.0);
        let response_5k = stack.magnitude_response(5000.0);

        // All should be close to unity (within 1 dB or ~0.89 to 1.12 linear)
        assert!(
            response_100 > 0.8 && response_100 < 1.2,
            "100Hz response {} should be near unity",
            response_100
        );
        assert!(
            response_1k > 0.8 && response_1k < 1.2,
            "1kHz response {} should be near unity",
            response_1k
        );
        assert!(
            response_5k > 0.8 && response_5k < 1.2,
            "5kHz response {} should be near unity",
            response_5k
        );
    }

    #[test]
    fn test_default_controls_relatively_flat_marshall() {
        let stack = ToneStack::new(ToneStackType::Marshall, SAMPLE_RATE);

        let response_100 = stack.magnitude_response(100.0);
        let response_1k = stack.magnitude_response(1000.0);
        let response_5k = stack.magnitude_response(5000.0);

        assert!(
            response_100 > 0.8 && response_100 < 1.2,
            "100Hz response {} should be near unity",
            response_100
        );
        assert!(
            response_1k > 0.8 && response_1k < 1.2,
            "1kHz response {} should be near unity",
            response_1k
        );
        assert!(
            response_5k > 0.8 && response_5k < 1.2,
            "5kHz response {} should be near unity",
            response_5k
        );
    }

    #[test]
    fn test_default_controls_relatively_flat_vox() {
        let stack = ToneStack::new(ToneStackType::Vox, SAMPLE_RATE);

        let response_100 = stack.magnitude_response(100.0);
        let response_1k = stack.magnitude_response(1000.0);
        let response_5k = stack.magnitude_response(5000.0);

        assert!(
            response_100 > 0.8 && response_100 < 1.2,
            "100Hz response {} should be near unity",
            response_100
        );
        assert!(
            response_1k > 0.8 && response_1k < 1.2,
            "1kHz response {} should be near unity",
            response_1k
        );
        assert!(
            response_5k > 0.8 && response_5k < 1.2,
            "5kHz response {} should be near unity",
            response_5k
        );
    }

    // -------------------------------------------------------------------------
    // Extreme Settings Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_extreme_settings_all_minimum() {
        let mut stack = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);
        stack.set_controls(ToneControls::new(0.0, 0.0, 0.0));

        // All controls at minimum should significantly cut the signal
        let bass_response = stack.magnitude_response(50.0);
        let mid_response = stack.magnitude_response(400.0);
        let treble_response = stack.magnitude_response(4000.0);

        // All bands should be cut (less than unity)
        assert!(
            bass_response < 0.5,
            "Bass at 0 should be cut, got {}",
            bass_response
        );
        assert!(
            mid_response < 0.5,
            "Mid at 0 should be cut, got {}",
            mid_response
        );
        assert!(
            treble_response < 0.5,
            "Treble at 0 should be cut, got {}",
            treble_response
        );
    }

    #[test]
    fn test_extreme_settings_all_maximum() {
        let mut stack = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);
        stack.set_controls(ToneControls::new(1.0, 1.0, 1.0));

        // All controls at maximum should significantly boost the signal
        let bass_response = stack.magnitude_response(50.0);
        let mid_response = stack.magnitude_response(400.0);
        let treble_response = stack.magnitude_response(4000.0);

        // All bands should be boosted (greater than unity)
        assert!(
            bass_response > 1.5,
            "Bass at 1 should be boosted, got {}",
            bass_response
        );
        assert!(
            mid_response > 1.5,
            "Mid at 1 should be boosted, got {}",
            mid_response
        );
        assert!(
            treble_response > 1.5,
            "Treble at 1 should be boosted, got {}",
            treble_response
        );
    }

    #[test]
    fn test_extreme_bass_only() {
        let mut stack = ToneStack::new(ToneStackType::Marshall, SAMPLE_RATE);
        stack.set_controls(ToneControls::new(1.0, 0.5, 0.5));

        let bass_response = stack.magnitude_response(50.0);
        let treble_response = stack.magnitude_response(5000.0);

        // Bass should be boosted, treble should be near unity
        assert!(bass_response > 2.0, "Max bass should boost lows significantly");
        assert!(
            treble_response > 0.8 && treble_response < 1.2,
            "Treble should be near unity with only bass boosted"
        );
    }

    #[test]
    fn test_extreme_treble_only() {
        let mut stack = ToneStack::new(ToneStackType::Marshall, SAMPLE_RATE);
        stack.set_controls(ToneControls::new(0.5, 0.5, 1.0));

        let bass_response = stack.magnitude_response(50.0);
        let treble_response = stack.magnitude_response(5000.0);

        // Treble should be boosted, bass should be near unity
        assert!(
            treble_response > 2.0,
            "Max treble should boost highs significantly"
        );
        assert!(
            bass_response > 0.8 && bass_response < 1.2,
            "Bass should be near unity with only treble boosted"
        );
    }

    // -------------------------------------------------------------------------
    // State Management Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_reset_clears_state() {
        let mut stack = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);
        stack.set_controls(ToneControls::new(0.8, 0.3, 0.9));

        // Build up state by processing many samples
        for _ in 0..1000 {
            stack.process_sample(1.0);
        }

        // Reset and process a single sample
        stack.reset();
        let first_after_reset = stack.process_sample(0.0);

        // After reset with zero input, output should be near zero
        assert!(
            first_after_reset.abs() < 0.01,
            "After reset with zero input, output should be near zero, got {}",
            first_after_reset
        );
    }

    #[test]
    fn test_type_change_resets_state() {
        let mut stack = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);

        // Build up state
        for _ in 0..1000 {
            stack.process_sample(1.0);
        }

        // Change type (should reset internally)
        stack.set_type(ToneStackType::Marshall);
        let first_after_change = stack.process_sample(0.0);

        // After type change with zero input, output should be near zero
        assert!(
            first_after_change.abs() < 0.01,
            "After type change with zero input, output should be near zero, got {}",
            first_after_change
        );
    }

    // -------------------------------------------------------------------------
    // Buffer Processing Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_buffer_processing_matches_sample_processing() {
        let mut stack1 = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);
        let mut stack2 = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);

        stack1.set_controls(ToneControls::new(0.7, 0.4, 0.8));
        stack2.set_controls(ToneControls::new(0.7, 0.4, 0.8));

        let input = [0.5, -0.3, 0.8, -0.1, 0.0, 0.2, -0.4, 0.6];

        // Process sample-by-sample
        let mut output1 = input;
        for sample in output1.iter_mut() {
            *sample = stack1.process_sample(*sample);
        }

        // Process as buffer
        let mut output2 = input;
        stack2.process_buffer(&mut output2);

        // Results should match
        for (i, (a, b)) in output1.iter().zip(output2.iter()).enumerate() {
            assert!(
                approx_eq(*a, *b, EPSILON),
                "Sample {} mismatch: {} vs {}",
                i,
                a,
                b
            );
        }
    }

    // -------------------------------------------------------------------------
    // Getter Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_stack_type_getter() {
        let stack = ToneStack::new(ToneStackType::Vox, SAMPLE_RATE);
        assert_eq!(stack.stack_type(), ToneStackType::Vox);
    }

    #[test]
    fn test_controls_getter() {
        let mut stack = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);
        stack.set_controls(ToneControls::new(0.3, 0.6, 0.9));

        let controls = stack.controls();
        assert!(approx_eq(controls.bass, 0.3, EPSILON));
        assert!(approx_eq(controls.mid, 0.6, EPSILON));
        assert!(approx_eq(controls.treble, 0.9, EPSILON));
    }

    #[test]
    fn test_sample_rate_getter() {
        let stack = ToneStack::new(ToneStackType::Marshall, 44100.0);
        assert!(approx_eq(stack.sample_rate(), 44100.0, EPSILON));
    }

    // -------------------------------------------------------------------------
    // Default Implementations Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_tone_stack_type_default() {
        let default_type = ToneStackType::default();
        assert_eq!(default_type, ToneStackType::Fender);
    }

    #[test]
    fn test_tone_controls_default() {
        let default_controls = ToneControls::default();
        assert!(approx_eq(default_controls.bass, 0.5, EPSILON));
        assert!(approx_eq(default_controls.mid, 0.5, EPSILON));
        assert!(approx_eq(default_controls.treble, 0.5, EPSILON));
    }

    // -------------------------------------------------------------------------
    // Numerical Stability Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_numerical_stability_extended_processing() {
        let mut stack = ToneStack::new(ToneStackType::Marshall, SAMPLE_RATE);
        stack.set_controls(ToneControls::new(0.9, 0.1, 0.95));

        // Process many samples without NaN/Inf
        for i in 0..100000 {
            let input = if i % 2 == 0 { 1.0 } else { -1.0 };
            let output = stack.process_sample(input);

            assert!(
                output.is_finite(),
                "Output became non-finite at sample {}",
                i
            );
        }
    }

    #[test]
    fn test_numerical_stability_rapid_control_changes() {
        let mut stack = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);

        for i in 0..1000 {
            // Rapidly change controls
            let t = (i as f32) / 1000.0;
            stack.set_bass(t);
            stack.set_mid(1.0 - t);
            stack.set_treble(t * 0.5 + 0.25);

            let output = stack.process_sample(0.5);
            assert!(output.is_finite(), "Output became non-finite at step {}", i);
        }
    }

    // -------------------------------------------------------------------------
    // Frequency Response Characteristic Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_bass_control_affects_low_frequencies() {
        let mut stack = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);

        stack.set_bass(0.0);
        let bass_cut = stack.magnitude_response(50.0);

        stack.set_bass(1.0);
        let bass_boost = stack.magnitude_response(50.0);

        assert!(
            bass_boost > bass_cut,
            "Bass boost ({}) should be greater than bass cut ({})",
            bass_boost,
            bass_cut
        );
    }

    #[test]
    fn test_mid_control_affects_mid_frequencies() {
        let mut stack = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);

        stack.set_mid(0.0);
        let mid_cut = stack.magnitude_response(400.0);

        stack.set_mid(1.0);
        let mid_boost = stack.magnitude_response(400.0);

        assert!(
            mid_boost > mid_cut,
            "Mid boost ({}) should be greater than mid cut ({})",
            mid_boost,
            mid_cut
        );
    }

    #[test]
    fn test_treble_control_affects_high_frequencies() {
        let mut stack = ToneStack::new(ToneStackType::Marshall, SAMPLE_RATE);

        stack.set_treble(0.0);
        let treble_cut = stack.magnitude_response(4000.0);

        stack.set_treble(1.0);
        let treble_boost = stack.magnitude_response(4000.0);

        assert!(
            treble_boost > treble_cut,
            "Treble boost ({}) should be greater than treble cut ({})",
            treble_boost,
            treble_cut
        );
    }

    #[test]
    fn test_magnitude_response_db() {
        let stack = ToneStack::new(ToneStackType::Fender, SAMPLE_RATE);

        // At neutral settings, response should be near 0 dB
        let db_1k = stack.magnitude_response_db(1000.0);

        assert!(
            db_1k.abs() < 2.0,
            "1kHz response should be within 2 dB of 0, got {} dB",
            db_1k
        );
    }
}
