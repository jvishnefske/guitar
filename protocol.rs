//! BLE Protocol Module
//!
//! Defines the communication protocol for phone app control.

use heapless::String;
use crate::fir_filter::FilterPreset;

/// BLE command from phone app
#[derive(Debug, Clone)]
pub enum BleCommand {
    SetFilterPreset(FilterPreset),
    SetVolume(u8),
    GetStatus,
    StartDiscovery,
    Connect([u8; 6]),
    Disconnect,
}

/// BLE response to phone app
#[derive(Debug, Clone)]
pub enum BleResponse {
    Ok,
    Error(String<64>),
    Status(SystemStatus),
}

/// System status for phone app
#[derive(Debug, Clone, Default)]
pub struct SystemStatus {
    pub usb_connected: bool,
    pub usb_streaming: bool,
    pub usb_buffer_level: u8,
    pub headphone_connected: bool,
    pub headphone_streaming: bool,
    pub headphone_name: String<32>,
    pub filter_preset: FilterPreset,
    pub filter_taps: u8,
    pub volume: u8,
    pub muted: bool,
    pub sample_rate: u32,
    pub latency_ms: u16,
    pub underruns: u32,
    pub overruns: u32,
}
