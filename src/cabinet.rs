//! Cabinet IR Convolution Module
//!
//! Convolves the signal with a speaker cabinet impulse response (IR)
//! to add realistic speaker coloration. This module implements time-domain
//! convolution using a circular delay line.
//!
//! # Architecture
//!
//! The cabinet simulation uses direct convolution rather than FFT-based methods
//! for several reasons:
//! - Deterministic latency (no FFT block boundaries)
//! - Lower memory footprint for short IRs (256-512 samples)
//! - Simpler implementation without FFT dependencies
//!
//! # CPU Budget
//!
//! This module consumes approximately 42.9% of the DSP CPU budget per the
//! specification. For a 512-sample IR at 48kHz, this is 512 multiply-accumulate
//! operations per sample.
//!
//! # Usage
//!
//! ```ignore
//! use guitar::cabinet::{Cabinet, CabinetIrData};
//!
//! let mut cabinet = Cabinet::new();
//! let ir = CabinetIrData::bypass();
//! cabinet.set_ir(ir);
//!
//! let output = cabinet.process_sample(0.5);
//! ```
//!
//! # Requirements Traceability
//!
//! - E6.1: Time-domain convolution engine (process_sample)
//! - E6.2: Support 256-512 sample IRs (MAX_IR_LENGTH, CabinetIrData::length)
//! - E6.3: Circular delay line implementation (delay_line, write_pos)
//! - E6.4: IR storage in flash, selectable at runtime (set_ir)

/// Maximum IR length in samples.
///
/// Supports IRs from 256 to 512 samples as specified in E6.2.
/// At 48kHz, 512 samples represents approximately 10.7ms of impulse response.
pub const MAX_IR_LENGTH: usize = 512;

/// Cabinet impulse response data.
///
/// IRs are stored as fixed-point or float arrays that can be embedded
/// in flash memory for embedded targets. The structure supports variable-length
/// IRs up to [`MAX_IR_LENGTH`] samples.
///
/// # Construction
///
/// IRs can be created via:
/// - [`CabinetIrData::new`] - From a slice of samples
/// - [`CabinetIrData::bypass`] - Unity gain passthrough
/// - [`create_test_ir`] - Simple exponential decay for testing
#[derive(Clone)]
pub struct CabinetIrData {
    /// Impulse response samples (up to 512).
    ///
    /// Only the first `length` samples are used in convolution.
    pub samples: [f32; MAX_IR_LENGTH],
    /// Actual length of the IR (may be less than MAX_IR_LENGTH).
    ///
    /// Valid range: 1 to [`MAX_IR_LENGTH`].
    pub length: usize,
    /// Descriptive name for UI display.
    pub name: &'static str,
}

impl CabinetIrData {
    /// Create a new IR from a slice of samples.
    ///
    /// If the input slice exceeds [`MAX_IR_LENGTH`], it will be truncated.
    ///
    /// # Arguments
    ///
    /// * `samples` - IR sample data
    /// * `name` - Descriptive name for the IR
    ///
    /// # Example
    ///
    /// ```ignore
    /// const MY_IR: CabinetIrData = CabinetIrData::new(&[1.0, 0.5, 0.25], "Test IR");
    /// ```
    pub const fn new(samples: &[f32], name: &'static str) -> Self {
        let mut ir_samples = [0.0f32; MAX_IR_LENGTH];
        let length = if samples.len() > MAX_IR_LENGTH {
            MAX_IR_LENGTH
        } else {
            samples.len()
        };
        let mut i = 0;
        while i < length {
            ir_samples[i] = samples[i];
            i += 1;
        }
        Self {
            samples: ir_samples,
            length,
            name,
        }
    }

    /// Create a bypass IR (impulse at sample 0).
    ///
    /// This IR passes the signal unchanged, useful for A/B comparison
    /// or when cabinet simulation is disabled.
    ///
    /// # Returns
    ///
    /// An IR with a single sample of value 1.0 at position 0.
    pub const fn bypass() -> Self {
        let mut samples = [0.0f32; MAX_IR_LENGTH];
        samples[0] = 1.0;
        Self {
            samples,
            length: 1,
            name: "Bypass",
        }
    }
}

/// Cabinet convolution processor.
///
/// Implements time-domain convolution using a circular delay line for
/// efficient sample-by-sample processing. The processor maintains internal
/// state and can be reset or have its IR changed at runtime.
///
/// # Performance
///
/// The [`process_sample`](Cabinet::process_sample) method is the hot path,
/// executing `ir.length` multiply-accumulate operations per sample.
/// For a 512-sample IR at 48kHz, this is approximately 24.6 million
/// multiply-adds per second.
pub struct Cabinet {
    /// Circular buffer storing recent input samples.
    delay_line: [f32; MAX_IR_LENGTH],
    /// Current impulse response data.
    ir: CabinetIrData,
    /// Current write position in the delay line (0 to MAX_IR_LENGTH-1).
    write_pos: usize,
}

impl Cabinet {
    /// Create a new cabinet with bypass IR.
    ///
    /// The cabinet starts in bypass mode with no coloration applied.
    /// Use [`set_ir`](Cabinet::set_ir) to load an impulse response.
    pub fn new() -> Self {
        Self {
            delay_line: [0.0; MAX_IR_LENGTH],
            ir: CabinetIrData::bypass(),
            write_pos: 0,
        }
    }

    /// Create a cabinet with a specific IR.
    ///
    /// # Arguments
    ///
    /// * `ir` - The impulse response to use
    pub fn with_ir(ir: CabinetIrData) -> Self {
        Self {
            delay_line: [0.0; MAX_IR_LENGTH],
            ir,
            write_pos: 0,
        }
    }

    /// Load a new impulse response.
    ///
    /// This also resets the delay line to prevent artifacts from
    /// the previous IR state.
    ///
    /// # Arguments
    ///
    /// * `ir` - The new impulse response to use
    pub fn set_ir(&mut self, ir: CabinetIrData) {
        self.ir = ir;
        self.reset();
    }

    /// Process a single sample through the cabinet simulation.
    ///
    /// This is the hot path - approximately 42.9% of CPU budget.
    /// The implementation uses a circular delay line and direct
    /// convolution for minimal latency.
    ///
    /// # Arguments
    ///
    /// * `x` - Input sample
    ///
    /// # Returns
    ///
    /// Convolved output sample with cabinet coloration applied.
    ///
    /// # Algorithm
    ///
    /// 1. Write input to delay line at current position
    /// 2. Compute dot product of delay line with IR (convolution sum)
    /// 3. Advance write position (circular wrap)
    #[inline]
    pub fn process_sample(&mut self, x: f32) -> f32 {
        // Write input to delay line at current position
        self.delay_line[self.write_pos] = x;

        // Convolve with IR using circular addressing
        let mut sum = 0.0f32;
        let ir_len = self.ir.length;

        for i in 0..ir_len {
            // Read from delay line in reverse order (newest to oldest)
            let read_pos = (self.write_pos + MAX_IR_LENGTH - i) % MAX_IR_LENGTH;
            sum += self.delay_line[read_pos] * self.ir.samples[i];
        }

        // Advance write position with wrap-around
        self.write_pos = (self.write_pos + 1) % MAX_IR_LENGTH;

        sum
    }

    /// Process a buffer in place.
    ///
    /// More efficient than calling [`process_sample`](Cabinet::process_sample)
    /// in a loop due to reduced function call overhead.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Audio buffer to process in place
    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Reset the delay line to zero.
    ///
    /// Call this when switching IRs or after a period of silence
    /// to prevent artifacts.
    pub fn reset(&mut self) {
        self.delay_line = [0.0; MAX_IR_LENGTH];
        self.write_pos = 0;
    }

    /// Get current IR name.
    ///
    /// Useful for UI display.
    pub fn ir_name(&self) -> &'static str {
        self.ir.name
    }

    /// Get current IR length in samples.
    pub fn ir_length(&self) -> usize {
        self.ir.length
    }
}

impl Default for Cabinet {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Test IR Generation
// ============================================================================

/// Create a simple exponential decay IR for testing.
///
/// This generates a normalized exponential decay impulse response.
/// Real cabinet IRs would be measured from actual speaker cabinets
/// and loaded from flash storage.
///
/// # Arguments
///
/// * `length` - Number of samples (clamped to [`MAX_IR_LENGTH`])
/// * `name` - Descriptive name for the IR
///
/// # Returns
///
/// A normalized IR with exponential decay characteristics.
pub fn create_test_ir(length: usize, name: &'static str) -> CabinetIrData {
    let mut samples = [0.0f32; MAX_IR_LENGTH];
    let len = length.min(MAX_IR_LENGTH);

    // Simple exponential decay (placeholder - real IRs are much more complex)
    for (i, sample) in samples[..len].iter_mut().enumerate() {
        let t = i as f32 / len as f32;
        // Exponential decay with linear taper
        *sample = libm::expf(-t * 4.0) * (1.0 - t);
    }

    // Normalize to unity DC gain
    let sum: f32 = samples[..len].iter().sum();
    if libm::fabsf(sum) > 0.001 {
        for s in samples[..len].iter_mut() {
            *s /= sum;
        }
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
    // CabinetIrData Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_ir_data_new() {
        let samples = [1.0, 0.5, 0.25, 0.125];
        let ir = CabinetIrData::new(&samples, "Test IR");

        assert_eq!(ir.length, 4);
        assert_eq!(ir.name, "Test IR");
        assert!(approx_eq(ir.samples[0], 1.0, 1e-6));
        assert!(approx_eq(ir.samples[1], 0.5, 1e-6));
        assert!(approx_eq(ir.samples[2], 0.25, 1e-6));
        assert!(approx_eq(ir.samples[3], 0.125, 1e-6));
    }

    #[test]
    fn test_ir_data_truncation() {
        // Create an IR longer than MAX_IR_LENGTH
        let long_samples: [f32; 600] = [0.5; 600];
        let ir = CabinetIrData::new(&long_samples, "Long IR");

        assert_eq!(ir.length, MAX_IR_LENGTH);
    }

    #[test]
    fn test_ir_data_bypass() {
        let ir = CabinetIrData::bypass();

        assert_eq!(ir.length, 1);
        assert_eq!(ir.name, "Bypass");
        assert!(approx_eq(ir.samples[0], 1.0, 1e-6));
        // All other samples should be zero
        for i in 1..MAX_IR_LENGTH {
            assert!(approx_eq(ir.samples[i], 0.0, 1e-6));
        }
    }

    // ------------------------------------------------------------------------
    // Bypass Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_bypass_passes_signal_unchanged() {
        let mut cabinet = Cabinet::new();

        // Process a series of samples
        let test_values = [0.0, 0.5, -0.5, 1.0, -1.0, 0.25, -0.25];
        for &input in &test_values {
            let output = cabinet.process_sample(input);
            assert!(
                approx_eq(output, input, 1e-6),
                "Bypass should pass signal unchanged: input={}, output={}",
                input,
                output
            );
        }
    }

    #[test]
    fn test_bypass_buffer_processing() {
        let mut cabinet = Cabinet::new();
        let mut buffer = [0.1, 0.2, 0.3, 0.4, 0.5];
        let expected = buffer;

        cabinet.process_buffer(&mut buffer);

        for (i, (&out, &exp)) in buffer.iter().zip(expected.iter()).enumerate() {
            assert!(
                approx_eq(out, exp, 1e-6),
                "Sample {}: expected {}, got {}",
                i,
                exp,
                out
            );
        }
    }

    // ------------------------------------------------------------------------
    // Impulse Response Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_impulse_produces_ir_output() {
        // Create a simple 4-sample IR
        let ir_samples = [0.5, 0.3, 0.15, 0.05];
        let ir = CabinetIrData::new(&ir_samples, "Test");
        let mut cabinet = Cabinet::with_ir(ir);

        // Send an impulse (single 1.0 followed by zeros)
        let out0 = cabinet.process_sample(1.0);
        let out1 = cabinet.process_sample(0.0);
        let out2 = cabinet.process_sample(0.0);
        let out3 = cabinet.process_sample(0.0);
        let out4 = cabinet.process_sample(0.0);

        // Output should be the IR itself
        assert!(
            approx_eq(out0, 0.5, 1e-6),
            "Sample 0: expected 0.5, got {}",
            out0
        );
        assert!(
            approx_eq(out1, 0.3, 1e-6),
            "Sample 1: expected 0.3, got {}",
            out1
        );
        assert!(
            approx_eq(out2, 0.15, 1e-6),
            "Sample 2: expected 0.15, got {}",
            out2
        );
        assert!(
            approx_eq(out3, 0.05, 1e-6),
            "Sample 3: expected 0.05, got {}",
            out3
        );
        assert!(
            approx_eq(out4, 0.0, 1e-6),
            "Sample 4: expected 0.0, got {}",
            out4
        );
    }

    #[test]
    fn test_convolution_linearity() {
        // Convolution should be linear: conv(a*x) = a * conv(x)
        let ir_samples = [0.5, 0.3, 0.15, 0.05];
        let ir = CabinetIrData::new(&ir_samples, "Test");

        let mut cabinet1 = Cabinet::with_ir(ir.clone());
        let mut cabinet2 = Cabinet::with_ir(ir);

        let scale = 2.5;

        // Process impulse at different scales
        let out1_0 = cabinet1.process_sample(1.0);
        let out2_0 = cabinet2.process_sample(scale);

        let out1_1 = cabinet1.process_sample(0.0);
        let out2_1 = cabinet2.process_sample(0.0);

        assert!(
            approx_eq(out2_0, out1_0 * scale, 1e-5),
            "Linearity violated at sample 0"
        );
        assert!(
            approx_eq(out2_1, out1_1 * scale, 1e-5),
            "Linearity violated at sample 1"
        );
    }

    // ------------------------------------------------------------------------
    // Convolution Sum Verification
    // ------------------------------------------------------------------------

    #[test]
    fn test_convolution_sum_correctness() {
        // Manual convolution verification
        // IR: [0.5, 0.3, 0.2]
        // Input: [1.0, 2.0, 3.0, 0.0, 0.0]
        // Expected output (linear convolution):
        //   y[0] = 1.0 * 0.5 = 0.5
        //   y[1] = 2.0 * 0.5 + 1.0 * 0.3 = 1.3
        //   y[2] = 3.0 * 0.5 + 2.0 * 0.3 + 1.0 * 0.2 = 2.3
        //   y[3] = 0.0 * 0.5 + 3.0 * 0.3 + 2.0 * 0.2 = 1.3
        //   y[4] = 0.0 * 0.5 + 0.0 * 0.3 + 3.0 * 0.2 = 0.6

        let ir_samples = [0.5, 0.3, 0.2];
        let ir = CabinetIrData::new(&ir_samples, "Test");
        let mut cabinet = Cabinet::with_ir(ir);

        let inputs = [1.0, 2.0, 3.0, 0.0, 0.0];
        let expected = [0.5, 1.3, 2.3, 1.3, 0.6];

        for (i, (&input, &exp)) in inputs.iter().zip(expected.iter()).enumerate() {
            let output = cabinet.process_sample(input);
            assert!(
                approx_eq(output, exp, 1e-5),
                "Sample {}: expected {}, got {}",
                i,
                exp,
                output
            );
        }
    }

    #[test]
    fn test_convolution_dc_response() {
        // A normalized IR should have unity DC gain
        let ir = create_test_ir(64, "Test64");
        let mut cabinet = Cabinet::with_ir(ir);

        // Process many samples of DC input
        let dc_value = 0.75;
        let mut last_output = 0.0;

        for _ in 0..200 {
            last_output = cabinet.process_sample(dc_value);
        }

        // After settling, output should equal input (unity DC gain)
        assert!(
            approx_eq(last_output, dc_value, 0.01),
            "DC response should be unity: expected {}, got {}",
            dc_value,
            last_output
        );
    }

    // ------------------------------------------------------------------------
    // Different IR Length Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_ir_length_256() {
        let ir = create_test_ir(256, "Test256");
        assert_eq!(ir.length, 256);

        let mut cabinet = Cabinet::with_ir(ir);
        assert_eq!(cabinet.ir_length(), 256);

        // Verify it processes without error
        for _ in 0..1000 {
            let _ = cabinet.process_sample(0.5);
        }
    }

    #[test]
    fn test_ir_length_384() {
        let ir = create_test_ir(384, "Test384");
        assert_eq!(ir.length, 384);

        let mut cabinet = Cabinet::with_ir(ir);
        assert_eq!(cabinet.ir_length(), 384);

        // Verify it processes without error
        for _ in 0..1000 {
            let _ = cabinet.process_sample(0.5);
        }
    }

    #[test]
    fn test_ir_length_512() {
        let ir = create_test_ir(512, "Test512");
        assert_eq!(ir.length, 512);

        let mut cabinet = Cabinet::with_ir(ir);
        assert_eq!(cabinet.ir_length(), 512);

        // Verify it processes without error
        for _ in 0..1000 {
            let _ = cabinet.process_sample(0.5);
        }
    }

    #[test]
    fn test_ir_length_minimum() {
        // Single sample IR (essentially just gain)
        let ir_samples = [0.8];
        let ir = CabinetIrData::new(&ir_samples, "Single");
        let mut cabinet = Cabinet::with_ir(ir);

        assert_eq!(cabinet.ir_length(), 1);

        let output = cabinet.process_sample(1.0);
        assert!(approx_eq(output, 0.8, 1e-6));
    }

    // ------------------------------------------------------------------------
    // Reset Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_reset_clears_delay_line() {
        let ir_samples = [0.5, 0.3, 0.2];
        let ir = CabinetIrData::new(&ir_samples, "Test");
        let mut cabinet = Cabinet::with_ir(ir);

        // Process some samples to fill delay line
        cabinet.process_sample(1.0);
        cabinet.process_sample(0.5);
        cabinet.process_sample(0.25);

        // Reset
        cabinet.reset();

        // After reset, processing zeros should produce zeros
        let out1 = cabinet.process_sample(0.0);
        let out2 = cabinet.process_sample(0.0);
        let out3 = cabinet.process_sample(0.0);

        assert!(
            approx_eq(out1, 0.0, 1e-6),
            "After reset, output should be 0: got {}",
            out1
        );
        assert!(
            approx_eq(out2, 0.0, 1e-6),
            "After reset, output should be 0: got {}",
            out2
        );
        assert!(
            approx_eq(out3, 0.0, 1e-6),
            "After reset, output should be 0: got {}",
            out3
        );
    }

    #[test]
    fn test_reset_allows_clean_restart() {
        let ir_samples = [0.5, 0.3, 0.2];
        let ir = CabinetIrData::new(&ir_samples, "Test");
        let mut cabinet = Cabinet::with_ir(ir);

        // First run
        let first_out = cabinet.process_sample(1.0);

        // Add more samples
        cabinet.process_sample(0.5);
        cabinet.process_sample(0.25);

        // Reset and run again
        cabinet.reset();
        let second_out = cabinet.process_sample(1.0);

        // Should get the same result
        assert!(
            approx_eq(first_out, second_out, 1e-6),
            "After reset, should get same result: {} vs {}",
            first_out,
            second_out
        );
    }

    // ------------------------------------------------------------------------
    // IR Loading Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_ir_loading_changes_behavior() {
        let mut cabinet = Cabinet::new();

        // Start with bypass
        assert_eq!(cabinet.ir_name(), "Bypass");
        let bypass_out = cabinet.process_sample(1.0);
        cabinet.reset();

        // Load a different IR
        let ir_samples = [0.5, 0.5];
        let new_ir = CabinetIrData::new(&ir_samples, "Half-Half");
        cabinet.set_ir(new_ir);

        assert_eq!(cabinet.ir_name(), "Half-Half");
        let new_out = cabinet.process_sample(1.0);

        // Outputs should be different
        assert!(
            !approx_eq(bypass_out, new_out, 1e-6),
            "Different IRs should produce different outputs"
        );
    }

    #[test]
    fn test_set_ir_resets_state() {
        let ir_samples = [0.5, 0.3, 0.2];
        let ir = CabinetIrData::new(&ir_samples, "Test");
        let mut cabinet = Cabinet::with_ir(ir.clone());

        // Fill delay line with data
        cabinet.process_sample(1.0);
        cabinet.process_sample(0.5);

        // Load new IR (which should reset)
        cabinet.set_ir(ir);

        // Processing zero should give zero (delay line was reset)
        let output = cabinet.process_sample(0.0);
        assert!(
            approx_eq(output, 0.0, 1e-6),
            "After set_ir, delay line should be cleared"
        );
    }

    // ------------------------------------------------------------------------
    // Default Implementation Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_default_creates_bypass() {
        let cabinet = Cabinet::default();
        assert_eq!(cabinet.ir_name(), "Bypass");
        assert_eq!(cabinet.ir_length(), 1);
    }

    // ------------------------------------------------------------------------
    // create_test_ir Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_create_test_ir_normalization() {
        let ir = create_test_ir(64, "Test");

        // Sum of IR samples should be approximately 1.0 (normalized)
        let sum: f32 = ir.samples[..ir.length].iter().sum();
        assert!(
            approx_eq(sum, 1.0, 0.01),
            "Test IR should be normalized: sum = {}",
            sum
        );
    }

    #[test]
    fn test_create_test_ir_decay() {
        let ir = create_test_ir(64, "Test");

        // First sample should be larger than last
        assert!(
            ir.samples[0] > ir.samples[ir.length - 1],
            "Test IR should decay over time"
        );
    }

    // ------------------------------------------------------------------------
    // Circular Buffer Wrap-Around Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_circular_buffer_wrap() {
        // Process many samples to ensure circular buffer wraps correctly
        let ir_samples = [0.5, 0.3, 0.15, 0.05];
        let ir = CabinetIrData::new(&ir_samples, "Test");
        let mut cabinet = Cabinet::with_ir(ir);

        // Process more samples than MAX_IR_LENGTH to ensure multiple wraps
        for i in 0..(MAX_IR_LENGTH * 3) {
            let input = if i % 100 == 0 { 1.0 } else { 0.0 };
            let output = cabinet.process_sample(input);
            // Just ensure no panic and output is finite
            assert!(output.is_finite(), "Output should be finite at sample {}", i);
        }
    }

    #[test]
    fn test_long_running_stability() {
        // Ensure numerical stability over many samples
        let ir = create_test_ir(256, "Test256");
        let mut cabinet = Cabinet::with_ir(ir);

        let mut max_output = 0.0f32;

        // Simulate 1 second at 48kHz
        for i in 0..48000 {
            // Sine wave input
            let t = i as f32 / 48000.0;
            let input = libm::sinf(2.0 * core::f32::consts::PI * 440.0 * t);
            let output = cabinet.process_sample(input);

            max_output = libm::fmaxf(max_output, libm::fabsf(output));

            // Output should remain bounded
            assert!(
                libm::fabsf(output) < 10.0,
                "Output unbounded at sample {}: {}",
                i,
                output
            );
        }

        // With unity DC gain IR and sine input, max should be around 1.0
        assert!(
            max_output < 2.0,
            "Max output should be reasonable: {}",
            max_output
        );
    }
}
