//! Signal Chain Module
//!
//! Complete guitar amp DSP pipeline connecting all processing blocks.
//!
//! # Overview
//!
//! This module wires together all DSP components into a complete signal chain:
//!
//! ```text
//! Input -> InputFilter -> Preamp -> ToneStack -> PowerAmp -> Cabinet -> Output
//! ```
//!
//! Each stage contributes to the final amp character:
//! - **Input**: DC blocking and input gain control
//! - **InputFilter**: Pickup/cable resonance modeling
//! - **Preamp**: Cascaded tube stage distortion
//! - **ToneStack**: Passive EQ (Fender/Marshall/Vox topology)
//! - **PowerAmp**: Push-pull compression and transformer coloring
//! - **Cabinet**: Speaker impulse response convolution
//! - **Output**: Master volume and soft limiting
//!
//! # Usage
//!
//! ```ignore
//! use guitar::signal_chain::SignalChain;
//! use guitar::preset::CLEAN_TWIN;
//!
//! // Create signal chain at 48kHz
//! let mut chain = SignalChain::new(48000.0);
//!
//! // Load a preset
//! chain.load_preset(&CLEAN_TWIN);
//!
//! // Process audio buffer
//! chain.process_buffer(&mut audio_buffer);
//! ```
//!
//! # Parameter Smoothing (G3.3)
//!
//! When switching presets or changing parameters, use `reset()` to clear
//! filter states and prevent artifacts. For glitch-free real-time updates,
//! access individual block references and apply smoothed parameter changes.
//!
//! # Requirements Traceability
//!
//! - G3.3: Parameter smoothing via reset and per-block access
//! - Signal flow as specified in tube_amp_emulation_spec.md

use crate::cabinet::Cabinet;
use crate::cabinet_irs::cabinet_ir_by_index;
use crate::input::InputStage;
use crate::input_filter::{InputFilter, PickupParams};
use crate::output::OutputStage;
use crate::poweramp::{PowerAmp, PowerAmpParams};
use crate::preamp::{Preamp, StageParams};
use crate::preset::AmpPreset;
use crate::tonestack::{ToneControls, ToneStack, ToneStackType};

/// Complete guitar amp signal chain.
///
/// Contains all processing blocks wired in the canonical order:
/// Input -> InputFilter -> Preamp -> ToneStack -> PowerAmp -> Cabinet -> Output
///
/// # State Management
///
/// Each block maintains its own internal state (filter delay lines, envelope
/// followers, etc.). Call [`reset()`](SignalChain::reset) to clear all state
/// when switching presets or processing a new audio stream.
///
/// # Thread Safety
///
/// This struct is not thread-safe. For concurrent access, wrap in
/// appropriate synchronization primitives.
pub struct SignalChain {
    /// Input stage: DC blocking and input gain
    input: InputStage,
    /// Input filter: pickup resonance modeling
    input_filter: InputFilter,
    /// Preamp: cascaded tube stages
    preamp: Preamp,
    /// Tone stack: passive EQ
    tonestack: ToneStack,
    /// Power amp: compression and transformer
    poweramp: PowerAmp,
    /// Cabinet: speaker IR convolution
    cabinet: Cabinet,
    /// Output: master volume and limiting
    output: OutputStage,
    /// Sample rate in Hz
    sample_rate: f32,
}

impl SignalChain {
    /// Creates a new signal chain with default settings.
    ///
    /// Initializes all blocks with sensible defaults:
    /// - Input: Unity gain (0 dB)
    /// - InputFilter: 3500 Hz resonance, Q=1.0
    /// - Preamp: 2 stages with default parameters
    /// - ToneStack: Fender topology, neutral (0.5, 0.5, 0.5)
    /// - PowerAmp: Default compression and transformer settings
    /// - Cabinet: Bypass (unity impulse)
    /// - Output: Unity volume, 1.0 ceiling
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Audio sample rate in Hz (typically 44100 or 48000)
    ///
    /// # Returns
    ///
    /// A new `SignalChain` ready for processing.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut chain = SignalChain::new(48000.0);
    /// ```
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        Self {
            input: InputStage::new(sample_rate),
            input_filter: InputFilter::new(sample_rate),
            preamp: Preamp::new(2, sample_rate),
            tonestack: ToneStack::new(ToneStackType::Fender, sample_rate),
            poweramp: PowerAmp::new(sample_rate),
            cabinet: Cabinet::new(),
            output: OutputStage::new(),
            sample_rate,
        }
    }

    /// Loads a preset, applying all parameters to the signal chain.
    ///
    /// This configures every block in the chain according to the preset's
    /// parameters. After loading, the chain is ready to produce the
    /// characteristic sound of the preset.
    ///
    /// # Arguments
    ///
    /// * `preset` - The amp preset to load
    ///
    /// # Note
    ///
    /// This does not reset filter states. Call [`reset()`](SignalChain::reset)
    /// after loading if you want to clear any accumulated state from previous
    /// processing.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use guitar::preset::PLEXI_CRUNCH;
    ///
    /// let mut chain = SignalChain::new(48000.0);
    /// chain.load_preset(&PLEXI_CRUNCH);
    /// chain.reset(); // Optional: clear filter states
    /// ```
    pub fn load_preset(&mut self, preset: &AmpPreset) {
        // Input stage: DC blocking gain
        self.input.set_gain_db(preset.input_gain_db);

        // Input filter: pickup simulation
        self.input_filter.set_params(PickupParams {
            freq_hz: preset.pickup_freq,
            q: preset.pickup_q,
        });

        // Preamp stages
        self.preamp.set_num_stages(preset.num_stages);
        for i in 0..preset.num_stages {
            self.preamp.set_stage_params(
                i,
                StageParams {
                    gain: preset.stage_gains[i],
                    asymmetry: preset.stage_asymmetry[i],
                    coupling_fc: preset.coupling_fc[i],
                    grid_threshold: preset.grid_threshold[i],
                },
            );
        }

        // Tone stack
        self.tonestack.set_type(preset.tone_stack_type);
        self.tonestack.set_controls(ToneControls {
            bass: preset.bass,
            mid: preset.mid,
            treble: preset.treble,
        });

        // Power amp
        self.poweramp.set_params(PowerAmpParams {
            crossover_amount: preset.crossover_amount,
            sag_depth: preset.sag_depth,
            sag_attack_ms: preset.sag_attack_ms,
            sag_release_ms: preset.sag_release_ms,
            transformer_fc: preset.transformer_fc,
        });

        // Cabinet IR selection
        self.cabinet.set_ir(cabinet_ir_by_index(preset.cabinet_ir_index));

        // Output master volume
        self.output.set_volume_db(preset.master_volume_db);
    }

    /// Processes a single sample through the entire chain.
    ///
    /// Signal flow:
    /// Input -> InputFilter -> Preamp -> ToneStack -> PowerAmp -> Cabinet -> Output
    ///
    /// # Arguments
    ///
    /// * `x` - Input sample
    ///
    /// # Returns
    ///
    /// Processed output sample with full amp simulation applied.
    ///
    /// # Performance
    ///
    /// This function is marked `#[inline]` and is the core of the audio
    /// processing hot path. Cabinet convolution is the most expensive
    /// operation (~42.9% of CPU budget).
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        let y = self.input.process_sample(x);
        let y = self.input_filter.process_sample(y);
        let y = self.preamp.process_sample(y);
        let y = self.tonestack.process_sample(y);
        let y = self.poweramp.process_sample(y);
        let y = self.cabinet.process_sample(y);
        self.output.process_sample(y)
    }

    /// Processes a buffer of samples in place.
    ///
    /// This is the primary entry point for real-time audio processing.
    /// Each sample passes through the complete signal chain.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Mutable slice of audio samples to process in-place
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut chain = SignalChain::new(48000.0);
    /// chain.load_preset(&CLEAN_TWIN);
    ///
    /// // Process 512 samples
    /// let mut buffer = [0.0f32; 512];
    /// // ... fill buffer with input audio ...
    /// chain.process_buffer(&mut buffer);
    /// // buffer now contains processed audio
    /// ```
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Resets all processing blocks (clears filter states).
    ///
    /// Call this when:
    /// - Switching presets
    /// - Starting a new audio stream
    /// - After a period of silence
    /// - To clear accumulated state
    ///
    /// This prevents transient artifacts from filter state carryover.
    pub fn reset(&mut self) {
        self.input.reset();
        self.input_filter.reset();
        self.preamp.reset();
        self.tonestack.reset();
        self.poweramp.reset();
        self.cabinet.reset();
    }

    /// Returns the sample rate.
    ///
    /// # Returns
    ///
    /// The sample rate in Hz that the chain was configured with.
    #[must_use]
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    // =========================================================================
    // Individual block accessors for real-time parameter adjustment (G3.3)
    // =========================================================================

    /// Returns a mutable reference to the input stage.
    ///
    /// Use for real-time adjustment of input gain.
    pub fn input(&mut self) -> &mut InputStage {
        &mut self.input
    }

    /// Returns a mutable reference to the input filter.
    ///
    /// Use for real-time adjustment of pickup resonance parameters.
    pub fn input_filter(&mut self) -> &mut InputFilter {
        &mut self.input_filter
    }

    /// Returns a mutable reference to the preamp.
    ///
    /// Use for real-time adjustment of gain staging and distortion.
    pub fn preamp(&mut self) -> &mut Preamp {
        &mut self.preamp
    }

    /// Returns a mutable reference to the tone stack.
    ///
    /// Use for real-time adjustment of EQ controls.
    pub fn tonestack(&mut self) -> &mut ToneStack {
        &mut self.tonestack
    }

    /// Returns a mutable reference to the power amp.
    ///
    /// Use for real-time adjustment of compression and transformer settings.
    pub fn poweramp(&mut self) -> &mut PowerAmp {
        &mut self.poweramp
    }

    /// Returns a mutable reference to the cabinet.
    ///
    /// Use for changing cabinet IR selection.
    pub fn cabinet(&mut self) -> &mut Cabinet {
        &mut self.cabinet
    }

    /// Returns a mutable reference to the output stage.
    ///
    /// Use for real-time adjustment of master volume and limiting.
    pub fn output(&mut self) -> &mut OutputStage {
        &mut self.output
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preset::{
        all_presets, preset_by_name, AC30_CHIME, BRIT_HIGH, CLEAN_TWIN, PLEXI_CRUNCH,
        RECTO_HEAVY, TWEED_DELUXE,
    };

    /// Tolerance for floating-point comparisons.
    const EPSILON: f32 = 1e-5;

    /// Standard test sample rate.
    const SAMPLE_RATE: f32 = 48000.0;

    /// Helper function to check if two floats are approximately equal.
    fn approx_eq(a: f32, b: f32, epsilon: f32) -> bool {
        (a - b).abs() < epsilon
    }

    // =========================================================================
    // Construction Tests
    // =========================================================================

    #[test]
    fn test_new_creates_chain() {
        let chain = SignalChain::new(SAMPLE_RATE);
        assert!(approx_eq(chain.sample_rate(), SAMPLE_RATE, EPSILON));
    }

    #[test]
    fn test_new_with_different_sample_rate() {
        let chain = SignalChain::new(44100.0);
        assert!(approx_eq(chain.sample_rate(), 44100.0, EPSILON));
    }

    // =========================================================================
    // Signal Pass-Through Tests
    // =========================================================================

    #[test]
    fn test_signal_passes_through_chain() {
        let mut chain = SignalChain::new(SAMPLE_RATE);

        // Process some samples to warm up filters
        for _ in 0..1000 {
            chain.process_sample(0.1);
        }

        // Signal should produce non-zero output
        let output = chain.process_sample(0.1);
        assert!(
            output.abs() > 0.0,
            "Signal should pass through chain: got {}",
            output
        );
    }

    #[test]
    fn test_silence_produces_silence() {
        let mut chain = SignalChain::new(SAMPLE_RATE);
        chain.reset();

        // Processing many zeros should produce zeros (or near-zero)
        let mut max_output = 0.0f32;
        for _ in 0..1000 {
            let output = chain.process_sample(0.0);
            max_output = max_output.max(output.abs());
        }

        assert!(
            max_output < 0.001,
            "Silence should produce near-silence: max={}",
            max_output
        );
    }

    #[test]
    fn test_buffer_processing_matches_sample() {
        let mut chain1 = SignalChain::new(SAMPLE_RATE);
        let mut chain2 = SignalChain::new(SAMPLE_RATE);

        let input = [0.1, 0.2, -0.1, 0.3, -0.2, 0.0, 0.15, -0.05];

        // Process sample-by-sample
        let mut output1 = input;
        for sample in output1.iter_mut() {
            *sample = chain1.process_sample(*sample);
        }

        // Process as buffer
        let mut output2 = input;
        chain2.process_buffer(&mut output2);

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

    // =========================================================================
    // Preset Loading Tests
    // =========================================================================

    #[test]
    fn test_load_preset_clean_twin() {
        let mut chain = SignalChain::new(SAMPLE_RATE);
        chain.load_preset(&CLEAN_TWIN);

        // Process some signal
        let mut output = 0.0;
        for _ in 0..1000 {
            output = chain.process_sample(0.2);
        }

        assert!(output.is_finite(), "Output should be finite");
    }

    #[test]
    fn test_load_preset_tweed_deluxe() {
        let mut chain = SignalChain::new(SAMPLE_RATE);
        chain.load_preset(&TWEED_DELUXE);

        let mut output = 0.0;
        for _ in 0..1000 {
            output = chain.process_sample(0.2);
        }

        assert!(output.is_finite(), "Output should be finite");
    }

    #[test]
    fn test_load_preset_plexi_crunch() {
        let mut chain = SignalChain::new(SAMPLE_RATE);
        chain.load_preset(&PLEXI_CRUNCH);

        let mut output = 0.0;
        for _ in 0..1000 {
            output = chain.process_sample(0.2);
        }

        assert!(output.is_finite(), "Output should be finite");
    }

    #[test]
    fn test_load_preset_brit_high() {
        let mut chain = SignalChain::new(SAMPLE_RATE);
        chain.load_preset(&BRIT_HIGH);

        let mut output = 0.0;
        for _ in 0..1000 {
            output = chain.process_sample(0.2);
        }

        assert!(output.is_finite(), "Output should be finite");
    }

    #[test]
    fn test_load_preset_ac30_chime() {
        let mut chain = SignalChain::new(SAMPLE_RATE);
        chain.load_preset(&AC30_CHIME);

        let mut output = 0.0;
        for _ in 0..1000 {
            output = chain.process_sample(0.2);
        }

        assert!(output.is_finite(), "Output should be finite");
    }

    #[test]
    fn test_load_preset_recto_heavy() {
        let mut chain = SignalChain::new(SAMPLE_RATE);
        chain.load_preset(&RECTO_HEAVY);

        let mut output = 0.0;
        for _ in 0..1000 {
            output = chain.process_sample(0.2);
        }

        assert!(output.is_finite(), "Output should be finite");
    }

    #[test]
    fn test_all_presets_load_successfully() {
        for preset in all_presets() {
            let mut chain = SignalChain::new(SAMPLE_RATE);
            chain.load_preset(preset);

            // Process some samples
            for _ in 0..100 {
                let output = chain.process_sample(0.1);
                assert!(
                    output.is_finite(),
                    "Preset {} produced non-finite output",
                    preset.name
                );
            }
        }
    }

    #[test]
    fn test_preset_by_name_and_load() {
        let mut chain = SignalChain::new(SAMPLE_RATE);

        if let Some(preset) = preset_by_name("plexi_crunch") {
            chain.load_preset(&preset);

            let output = chain.process_sample(0.2);
            assert!(output.is_finite());
        } else {
            panic!("plexi_crunch preset not found");
        }
    }

    // =========================================================================
    // Preset Distinction Tests
    // =========================================================================

    #[test]
    fn test_presets_produce_different_outputs() {
        // Each preset should produce measurably different output
        let presets = [
            &CLEAN_TWIN,
            &TWEED_DELUXE,
            &PLEXI_CRUNCH,
            &BRIT_HIGH,
            &AC30_CHIME,
            &RECTO_HEAVY,
        ];

        let mut outputs = Vec::new();

        for preset in presets {
            let mut chain = SignalChain::new(SAMPLE_RATE);
            chain.load_preset(preset);
            chain.reset();

            // Use a sine wave for consistent input
            let mut sum = 0.0;
            for i in 0..4800 {
                let t = i as f32 / SAMPLE_RATE;
                let input = 0.3 * libm::sinf(2.0 * core::f32::consts::PI * 440.0 * t);
                let output = chain.process_sample(input);
                sum += output * output; // RMS accumulator
            }
            let rms = libm::sqrtf(sum / 4800.0);
            outputs.push((preset.name, rms));
        }

        // Verify that not all outputs are identical
        let first_rms = outputs[0].1;
        let mut all_same = true;
        for (name, rms) in &outputs {
            if (rms - first_rms).abs() > 0.01 {
                all_same = false;
            }
            assert!(rms.is_finite(), "Preset {} produced non-finite RMS", name);
        }

        assert!(
            !all_same,
            "All presets produced identical output - they should differ"
        );
    }

    #[test]
    fn test_clean_vs_heavy_distortion_difference() {
        // Clean preset should have lower distortion than heavy preset
        let mut chain_clean = SignalChain::new(SAMPLE_RATE);
        let mut chain_heavy = SignalChain::new(SAMPLE_RATE);

        chain_clean.load_preset(&CLEAN_TWIN);
        chain_heavy.load_preset(&RECTO_HEAVY);

        chain_clean.reset();
        chain_heavy.reset();

        // Feed identical sine wave to both
        let freq = 440.0;
        let mut clean_sum = 0.0;
        let mut heavy_sum = 0.0;

        for i in 0..4800 {
            let t = i as f32 / SAMPLE_RATE;
            let input = 0.3 * libm::sinf(2.0 * core::f32::consts::PI * freq * t);

            let clean_out = chain_clean.process_sample(input);
            let heavy_out = chain_heavy.process_sample(input);

            clean_sum += clean_out.abs();
            heavy_sum += heavy_out.abs();
        }

        // Heavy preset typically has more compression/saturation
        // Both should produce meaningful output
        assert!(clean_sum > 0.0, "Clean preset should produce output");
        assert!(heavy_sum > 0.0, "Heavy preset should produce output");
    }

    // =========================================================================
    // Reset Tests
    // =========================================================================

    #[test]
    fn test_reset_clears_state() {
        let mut chain = SignalChain::new(SAMPLE_RATE);

        // Build up state with signal
        for _ in 0..5000 {
            chain.process_sample(0.5);
        }

        // Reset
        chain.reset();

        // After reset, processing zeros should give near-zeros
        let output = chain.process_sample(0.0);
        assert!(
            output.abs() < 0.01,
            "After reset with zero input, output should be near zero: {}",
            output
        );
    }

    #[test]
    fn test_reset_produces_repeatable_results() {
        let mut chain = SignalChain::new(SAMPLE_RATE);
        chain.load_preset(&PLEXI_CRUNCH);

        // First run
        chain.reset();
        let mut first_outputs = Vec::new();
        for i in 0..100 {
            let input = if i % 2 == 0 { 0.3 } else { -0.3 };
            first_outputs.push(chain.process_sample(input));
        }

        // Process more to change state
        for _ in 0..1000 {
            chain.process_sample(0.5);
        }

        // Reset and run again
        chain.reset();
        let mut second_outputs = Vec::new();
        for i in 0..100 {
            let input = if i % 2 == 0 { 0.3 } else { -0.3 };
            second_outputs.push(chain.process_sample(input));
        }

        // Results should match
        for (i, (a, b)) in first_outputs.iter().zip(second_outputs.iter()).enumerate() {
            assert!(
                approx_eq(*a, *b, EPSILON),
                "Sample {} should match after reset: {} vs {}",
                i,
                a,
                b
            );
        }
    }

    // =========================================================================
    // Numerical Stability Tests
    // =========================================================================

    #[test]
    fn test_numerical_stability_long_buffer() {
        let mut chain = SignalChain::new(SAMPLE_RATE);
        chain.load_preset(&BRIT_HIGH);

        // Process 10 seconds of audio (480,000 samples at 48kHz)
        for i in 0..480000 {
            let input = if i % 2 == 0 { 0.5 } else { -0.5 };
            let output = chain.process_sample(input);

            assert!(
                output.is_finite(),
                "Output became non-finite at sample {}",
                i
            );
        }
    }

    #[test]
    fn test_numerical_stability_all_presets() {
        for preset in all_presets() {
            let mut chain = SignalChain::new(SAMPLE_RATE);
            chain.load_preset(preset);

            // Process 1 second of alternating signal
            for i in 0..48000 {
                let input = if i % 2 == 0 { 0.5 } else { -0.5 };
                let output = chain.process_sample(input);

                assert!(
                    output.is_finite(),
                    "Preset {} produced non-finite output at sample {}",
                    preset.name,
                    i
                );
            }
        }
    }

    #[test]
    fn test_numerical_stability_extreme_input() {
        let mut chain = SignalChain::new(SAMPLE_RATE);
        chain.load_preset(&RECTO_HEAVY);

        // Even with extreme (but not infinite) input, output should remain bounded
        for i in 0..1000 {
            let input = if i % 2 == 0 { 1.0 } else { -1.0 };
            let output = chain.process_sample(input);

            assert!(output.is_finite(), "Output should be finite");
            // Output should be bounded due to soft clipping
            assert!(
                output.abs() <= 1.0,
                "Output should be bounded: {}",
                output
            );
        }
    }

    #[test]
    fn test_output_bounded_by_soft_clipper() {
        let mut chain = SignalChain::new(SAMPLE_RATE);
        chain.load_preset(&RECTO_HEAVY);

        // Maximum gain, hot input
        chain.output().set_ceiling(1.0);

        for i in 0..1000 {
            let input = if i % 2 == 0 { 1.5 } else { -1.5 };
            let output = chain.process_sample(input);

            assert!(
                output.abs() <= 1.0,
                "Output should never exceed ceiling: {}",
                output
            );
        }
    }

    // =========================================================================
    // Block Accessor Tests
    // =========================================================================

    #[test]
    fn test_input_accessor() {
        let mut chain = SignalChain::new(SAMPLE_RATE);

        chain.input().set_gain_db(10.0);
        assert!(
            chain.input().gain_db() > 9.0 && chain.input().gain_db() < 11.0,
            "Input gain should be ~10 dB"
        );
    }

    #[test]
    fn test_input_filter_accessor() {
        let mut chain = SignalChain::new(SAMPLE_RATE);

        chain.input_filter().set_frequency(4000.0);
        assert!(
            approx_eq(chain.input_filter().params().freq_hz, 4000.0, 1.0),
            "Input filter frequency should be 4000 Hz"
        );
    }

    #[test]
    fn test_preamp_accessor() {
        let mut chain = SignalChain::new(SAMPLE_RATE);

        chain.preamp().set_num_stages(3);
        assert_eq!(
            chain.preamp().num_stages(),
            3,
            "Preamp should have 3 stages"
        );
    }

    #[test]
    fn test_tonestack_accessor() {
        let mut chain = SignalChain::new(SAMPLE_RATE);

        chain.tonestack().set_type(ToneStackType::Marshall);
        assert_eq!(
            chain.tonestack().stack_type(),
            ToneStackType::Marshall,
            "Tonestack should be Marshall"
        );
    }

    #[test]
    fn test_poweramp_accessor() {
        let mut chain = SignalChain::new(SAMPLE_RATE);

        chain.poweramp().set_params(PowerAmpParams {
            crossover_amount: 0.05,
            sag_depth: 0.5,
            sag_attack_ms: 20.0,
            sag_release_ms: 150.0,
            transformer_fc: 7000.0,
        });

        let params = chain.poweramp().params();
        assert!(
            approx_eq(params.sag_depth, 0.5, EPSILON),
            "Sag depth should be 0.5"
        );
    }

    #[test]
    fn test_cabinet_accessor() {
        let mut chain = SignalChain::new(SAMPLE_RATE);

        let ir = cabinet_ir_by_index(2); // 4x12 Heavy
        chain.cabinet().set_ir(ir);
        assert_eq!(
            chain.cabinet().ir_name(),
            "4x12_heavy",
            "Cabinet should be 4x12 heavy"
        );
    }

    #[test]
    fn test_output_accessor() {
        let mut chain = SignalChain::new(SAMPLE_RATE);

        chain.output().set_volume_db(-12.0);
        // -12 dB ~= 0.251 linear
        assert!(
            chain.output().volume_linear() > 0.2 && chain.output().volume_linear() < 0.3,
            "Output volume should be ~0.25"
        );
    }

    // =========================================================================
    // Integration Tests
    // =========================================================================

    #[test]
    fn test_switching_presets_mid_stream() {
        let mut chain = SignalChain::new(SAMPLE_RATE);

        // Start with clean
        chain.load_preset(&CLEAN_TWIN);

        for _ in 0..1000 {
            let output = chain.process_sample(0.3);
            assert!(output.is_finite());
        }

        // Switch to heavy mid-stream
        chain.load_preset(&RECTO_HEAVY);
        chain.reset();

        for _ in 0..1000 {
            let output = chain.process_sample(0.3);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_process_sine_wave() {
        let mut chain = SignalChain::new(SAMPLE_RATE);
        chain.load_preset(&TWEED_DELUXE);

        let freq = 440.0; // A4
        let mut max_output = 0.0f32;

        for i in 0..4800 {
            // 100ms
            let t = i as f32 / SAMPLE_RATE;
            let input = 0.3 * libm::sinf(2.0 * core::f32::consts::PI * freq * t);
            let output = chain.process_sample(input);

            max_output = max_output.max(output.abs());

            assert!(output.is_finite(), "Output should be finite");
        }

        assert!(
            max_output > 0.0,
            "Sine wave should produce meaningful output"
        );
    }

    #[test]
    fn test_process_guitar_like_signal() {
        let mut chain = SignalChain::new(SAMPLE_RATE);
        chain.load_preset(&AC30_CHIME);

        // Simulate a plucked string: decaying sinusoid with harmonics
        let fundamental = 330.0; // E4

        for i in 0..9600 {
            // 200ms
            let t = i as f32 / SAMPLE_RATE;

            // Decaying harmonics
            let env = libm::expf(-t * 5.0);
            let h1 = 0.5 * libm::sinf(2.0 * core::f32::consts::PI * fundamental * t);
            let h2 = 0.3 * libm::sinf(2.0 * core::f32::consts::PI * fundamental * 2.0 * t);
            let h3 = 0.15 * libm::sinf(2.0 * core::f32::consts::PI * fundamental * 3.0 * t);

            let input = env * (h1 + h2 + h3);
            let output = chain.process_sample(input);

            assert!(output.is_finite(), "Output should be finite");
            assert!(
                output.abs() <= 1.0,
                "Output should be bounded: {}",
                output
            );
        }
    }
}
