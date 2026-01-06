//! Bluetooth Module (A2DP + BLE)
//!
//! Stub implementation for ESP32-S3 Bluetooth functionality.

use heapless::String;

/// A2DP connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum A2dpState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Streaming,
}

/// BLE connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BleState {
    #[default]
    Idle,
    Advertising,
    Connected,
}

/// Bluetooth status
#[derive(Debug, Default)]
pub struct BluetoothStatus {
    pub a2dp_state: A2dpState,
    pub ble_state: BleState,
    pub headphone_connected: bool,
    pub streaming: bool,
    pub volume: u8,
}

/// Bluetooth manager for A2DP and BLE
pub struct BluetoothManager {
    status: BluetoothStatus,
    pending_params: Option<crate::fir_filter::FilterParams>,
}

impl BluetoothManager {
    pub fn new() -> Self {
        Self {
            status: BluetoothStatus::default(),
            pending_params: None,
        }
    }

    pub fn init(&mut self) -> Result<(), &'static str> {
        log::info!("Bluetooth: Initializing (stub)");
        Ok(())
    }

    pub fn is_streaming(&self) -> bool {
        self.status.streaming
    }

    pub fn send_audio(&mut self, _samples: &[i16]) -> Result<(), &'static str> {
        // Stub: Would send audio over A2DP
        Ok(())
    }

    pub fn take_pending_params(&mut self) -> Option<crate::fir_filter::FilterParams> {
        self.pending_params.take()
    }

    pub fn get_status(&self) -> &BluetoothStatus {
        &self.status
    }
}

impl Default for BluetoothManager {
    fn default() -> Self {
        Self::new()
    }
}
