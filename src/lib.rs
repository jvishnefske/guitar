//! Guitar Amp DSP Library
//!
//! Tube amplifier emulation for embedded systems.

#![cfg_attr(not(feature = "std"), no_std)]

// Core DSP primitives (Wave 1)
pub mod biquad;
pub mod dsp_math;

// Signal chain blocks (Wave 2-4)
pub mod input;
pub mod input_filter;
pub mod output;
pub mod preamp;
pub mod poweramp;
pub mod cabinet;
pub mod cabinet_irs;
pub mod tonestack;

// Preset system (Wave 5)
pub mod preset;

// Signal chain (complete pipeline)
pub mod signal_chain;

// Existing filter (already implemented)
// pub mod fir_filter;
