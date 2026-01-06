//! ESP32-S3 Guitar Amp DSP Firmware
//!
//! Real-time tube amp emulation running on ESP32-S3.
//!
//! # Signal Flow
//! ```text
//! USB Audio In -> DSP (Tube Amp Emulation) -> Bluetooth A2DP Out
//!                         ^
//!                    BLE Control
//!                         ^
//!                    Phone App
//! ```

#![no_std]
#![no_main]

extern crate alloc;

use core::sync::atomic::{AtomicBool, Ordering};

use esp_idf_svc::hal::prelude::Peripherals;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::sys as esp_idf_sys;

use guitar_amp_dsp::signal_chain::SignalChain;
use guitar_amp_dsp::preset::{self, AmpPreset};

// Global allocator
#[global_allocator]
static ALLOCATOR: esp_alloc::EspHeap = esp_alloc::EspHeap::empty();

/// Initialize heap allocator
fn init_heap() {
    const HEAP_SIZE: usize = 64 * 1024; // 64KB heap
    static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
    unsafe {
        ALLOCATOR.init(HEAP.as_mut_ptr(), HEAP_SIZE);
    }
}

/// System shutdown flag
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Application entry point
#[esp_idf_svc::hal::entry]
fn main() -> ! {
    // Initialize ESP-IDF
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    init_heap();

    log::info!("===============================================");
    log::info!("  ESP32-S3 Guitar Amp DSP");
    log::info!("===============================================");
    log::info!("Firmware version: {}", env!("CARGO_PKG_VERSION"));

    // Get peripherals
    let _peripherals = Peripherals::take().expect("Failed to take peripherals");

    // Print system info
    print_system_info();

    // Initialize DSP signal chain at 48kHz
    let sample_rate = 48000.0;
    let mut signal_chain = SignalChain::new(sample_rate);

    // Load default preset (Clean Twin)
    let default_preset = preset::CLEAN_TWIN;
    signal_chain.load_preset(&default_preset);
    log::info!("Loaded preset: {}", default_preset.name);

    // List available presets
    log::info!("Available presets:");
    for (i, preset) in preset::all_presets().iter().enumerate() {
        log::info!("  {}: {} ({} stages)", i, preset.name, preset.num_stages);
    }

    // Demo: Process some test samples
    demo_dsp_processing(&mut signal_chain);

    log::info!("DSP system ready!");
    log::info!("Note: USB Audio and Bluetooth modules not yet implemented");

    // Main loop (placeholder - would handle real audio)
    loop {
        if SHUTDOWN.load(Ordering::Relaxed) {
            log::info!("Shutdown requested");
            break;
        }

        // In a real implementation:
        // 1. Get audio from USB
        // 2. Process through signal_chain
        // 3. Send to Bluetooth

        // For now, just idle
        unsafe {
            esp_idf_sys::vTaskDelay(100);
        }
    }

    log::info!("Main loop exiting");

    // Should not reach here
    loop {
        unsafe {
            esp_idf_sys::vTaskDelay(1000);
        }
    }
}

/// Demo DSP processing with test signal
fn demo_dsp_processing(chain: &mut SignalChain) {
    log::info!("Running DSP demo...");

    // Create a simple test signal (440Hz sine wave, 1ms at 48kHz = 48 samples)
    let mut test_buffer = [0.0f32; 48];
    for (i, sample) in test_buffer.iter_mut().enumerate() {
        let t = i as f32 / 48000.0;
        *sample = libm::sinf(2.0 * core::f32::consts::PI * 440.0 * t) * 0.5;
    }

    // Process through DSP
    let input_rms = rms(&test_buffer);
    chain.process_buffer(&mut test_buffer);
    let output_rms = rms(&test_buffer);

    log::info!("  Input RMS: {:.4}", input_rms);
    log::info!("  Output RMS: {:.4}", output_rms);
    log::info!("  Gain: {:.2} dB", 20.0 * libm::log10f(output_rms / input_rms.max(0.0001)));

    // Test all presets
    log::info!("Testing all presets:");
    for preset in preset::all_presets() {
        chain.load_preset(preset);

        // Reset test signal
        for (i, sample) in test_buffer.iter_mut().enumerate() {
            let t = i as f32 / 48000.0;
            *sample = libm::sinf(2.0 * core::f32::consts::PI * 440.0 * t) * 0.5;
        }

        chain.process_buffer(&mut test_buffer);
        let rms_out = rms(&test_buffer);
        log::info!("  {}: RMS={:.4}", preset.name, rms_out);
    }

    log::info!("DSP demo complete!");
}

/// Calculate RMS of a buffer
fn rms(buffer: &[f32]) -> f32 {
    let sum_sq: f32 = buffer.iter().map(|x| x * x).sum();
    libm::sqrtf(sum_sq / buffer.len() as f32)
}

/// Print system information
fn print_system_info() {
    log::info!("System Information:");
    log::info!("  Chip: ESP32-S3");

    unsafe {
        let info = esp_idf_sys::esp_get_idf_version();
        if !info.is_null() {
            let version = core::ffi::CStr::from_ptr(info);
            log::info!("  ESP-IDF: {:?}", version);
        }

        log::info!("  Free heap: {} bytes", esp_idf_sys::esp_get_free_heap_size());
    }
}

/// Panic handler - reset device
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("PANIC: {}", info);

    // Short delay then restart
    unsafe {
        esp_idf_sys::vTaskDelay(100);
        esp_idf_sys::esp_restart();
    }

    // Never reaches here
    loop {}
}
