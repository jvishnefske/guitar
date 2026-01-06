//! ESP32-S3 USB Audio DSP with Bluetooth
//!
//! This firmware implements a USB speaker that processes audio through
//! a configurable FIR filter and streams it to Bluetooth headphones.
//! A phone app controls filter parameters via BLE.
//!
//! # Hardware Requirements
//! - ESP32-S3 DevKit (with USB OTG support)
//! - USB-C cable for audio input
//! - Bluetooth headphones (A2DP sink)
//!
//! # Audio Pipeline
//! ```text
//! USB Host (PC) ──USB Audio──→ ESP32-S3 ──FIR Filter──→ A2DP ──→ Headphones
//!                                  ↑
//!                           BLE Control
//!                                  ↑
//!                            Phone App
//! ```

#![no_std]
#![no_main]

extern crate alloc;

mod bluetooth;
mod fir_filter;
mod protocol;
mod usb_audio;

use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, Ordering};

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};

use esp_idf_hal::prelude::*;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_sys as _;

use heapless::spsc::Queue;

use crate::bluetooth::{A2dpState, BleState, BluetoothManager};
use crate::fir_filter::{FilterParams, FirFilter};
use crate::protocol::{BleResponse, SystemStatus};
use crate::usb_audio::{AudioBuffer, UsbAudioDevice, NUM_BUFFERS};

// Global allocator for heap usage
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

/// Audio sample buffer queue (USB → DSP)
static mut USB_RX_QUEUE: Queue<AudioBuffer, NUM_BUFFERS> = Queue::new();

/// Filter parameter update channel
static FILTER_UPDATE: Signal<CriticalSectionRawMutex, FilterParams> = Signal::new();

/// System shutdown flag
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Application entry point
#[esp_idf_svc::hal::entry]
fn main() -> ! {
    // Initialize ESP-IDF
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    
    init_heap();
    
    log::info!("===============================================");
    log::info!("  ESP32-S3 USB Audio DSP with Bluetooth");
    log::info!("===============================================");
    log::info!("Firmware version: {}", env!("CARGO_PKG_VERSION"));
    
    // Get peripherals
    let peripherals = Peripherals::take().expect("Failed to take peripherals");
    
    // Initialize system event loop
    let _sysloop = EspSystemEventLoop::take().expect("Failed to take event loop");
    
    // Run the async executor
    let executor = embassy_executor::Executor::new();
    let executor = unsafe { &*(&executor as *const _) };
    
    executor.run(|spawner| {
        spawner.spawn(main_task()).ok();
    })
}

/// Main application task
#[embassy_executor::task]
async fn main_task() {
    log::info!("Starting main task...");
    
    // Initialize components
    let mut filter = FirFilter::new();
    
    // Initialize USB Audio
    let usb_queue = unsafe { &mut USB_RX_QUEUE };
    let mut usb_audio = UsbAudioDevice::new(usb_queue);
    if let Err(e) = usb_audio.init() {
        log::error!("USB Audio init failed: {}", e);
    }
    
    // Initialize Bluetooth
    let mut bt_manager = BluetoothManager::new();
    if let Err(e) = bt_manager.init() {
        log::error!("Bluetooth init failed: {}", e);
    }
    
    log::info!("All components initialized, entering main loop");
    
    // Main processing loop
    loop {
        // Check for shutdown
        if SHUTDOWN.load(Ordering::Relaxed) {
            log::info!("Shutdown requested");
            break;
        }
        
        // Check for filter parameter updates from BLE
        if let Some(params) = bt_manager.take_pending_params() {
            log::info!("Updating filter: {:?}", params.preset);
            filter.update_params(&params);
        }
        
        // Process audio if available
        if let Some(mut buffer) = usb_audio.get_audio_buffer() {
            // Apply volume
            usb_audio.apply_volume(&mut buffer);
            
            // Apply FIR filter
            filter.process_stereo(&mut buffer.samples[..buffer.valid_samples]);
            
            // Send to Bluetooth headphones
            if bt_manager.is_streaming() {
                if let Err(e) = bt_manager.send_audio(&buffer.samples[..buffer.valid_samples]) {
                    log::warn!("BT send error: {:?}", e);
                }
            }
        }
        
        // Yield to other tasks
        Timer::after(Duration::from_micros(100)).await;
    }
    
    log::info!("Main task exiting");
}

/// Status reporting task - sends periodic updates to phone app
#[embassy_executor::task]
async fn status_task(bt_manager: &'static RefCell<BluetoothManager>) {
    loop {
        Timer::after(Duration::from_secs(1)).await;
        
        // Build status message
        let bt_status = bt_manager.borrow().get_status();
        let status = SystemStatus {
            usb_connected: true, // TODO: Get from USB driver
            usb_streaming: true,
            usb_buffer_level: 50,
            headphone_connected: bt_status.headphone_connected,
            headphone_streaming: bt_status.streaming,
            headphone_name: heapless::String::new(),
            filter_preset: crate::fir_filter::FilterPreset::Bypass,
            filter_taps: 1,
            volume: bt_status.volume,
            muted: false,
            sample_rate: 48000,
            latency_ms: 50,
            underruns: 0,
            overruns: 0,
        };
        
        // Send to connected phone (if any)
        // This would use the BLE notify mechanism
        let _ = status; // TODO: Send via BLE notification
    }
}

/// Panic handler
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("PANIC: {}", info);
    
    // Reset the device after a short delay
    unsafe {
        esp_idf_sys::esp_restart();
    }
}

/// Configuration constants
pub mod config {
    /// Audio processing buffer size in milliseconds
    pub const BUFFER_MS: u32 = 10;
    
    /// Target audio latency in milliseconds
    pub const TARGET_LATENCY_MS: u32 = 50;
    
    /// Maximum acceptable latency before dropping frames
    pub const MAX_LATENCY_MS: u32 = 100;
    
    /// BLE advertising interval (ms)
    pub const BLE_ADV_INTERVAL_MS: u32 = 100;
    
    /// A2DP reconnection timeout (seconds)
    pub const A2DP_RECONNECT_TIMEOUT_S: u32 = 30;
}

/// Runtime statistics for monitoring
#[derive(Debug, Default)]
pub struct RuntimeStats {
    /// Total audio frames processed
    pub frames_processed: u64,
    /// USB buffer underruns
    pub usb_underruns: u32,
    /// USB buffer overruns
    pub usb_overruns: u32,
    /// Bluetooth buffer underruns
    pub bt_underruns: u32,
    /// Maximum observed latency (us)
    pub max_latency_us: u32,
    /// CPU usage estimate (0-100)
    pub cpu_usage: u8,
}

/// Print system info on startup
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
        log::info!("  Min free heap: {} bytes", esp_idf_sys::esp_get_minimum_free_heap_size());
    }
}
