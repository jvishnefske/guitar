# ESP32-S3 USB Audio DSP with Bluetooth

A Rust embedded project that turns an ESP32-S3 into a USB speaker with real-time FIR filtering
and Bluetooth streaming to headphones, controlled by a phone app via BLE.

## Features

- **USB Audio Class Device**: Appears as a USB speaker to your computer
- **FIR Filter DSP**: Configurable low-pass, high-pass, band-pass, parametric EQ, or custom coefficients
- **Bluetooth A2DP Source**: Streams processed audio to any Bluetooth headphones
- **BLE Control**: Phone app can adjust filter parameters in real-time
- **Low Latency**: Optimized for ~50ms end-to-end latency

## Hardware Requirements

| Component | Requirement |
|-----------|-------------|
| MCU | **ESP32-S3** (required for USB OTG + Bluetooth) |
| Flash | 4MB minimum |
| USB | USB-C or Micro-USB with OTG support |

> ⚠️ **Important**: This project requires ESP32-S3. Other ESP32 variants will NOT work:
> - ESP32 (original): No USB device mode
> - ESP32-S2: No Bluetooth
> - ESP32-C3/C6: No USB OTG

### Tested Boards

- ESP32-S3-DevKitC-1
- ESP32-S3-WROOM-1
- Unexpected Maker TinyS3

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Phone App                                │
│                    (Filter Control UI)                           │
└─────────────────────────────┬───────────────────────────────────┘
                              │ BLE GATT
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                         ESP32-S3                                 │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
│  │   USB    │───▶│   FIR    │───▶│   SBC    │───▶│  A2DP    │  │
│  │  Audio   │    │  Filter  │    │ Encoder  │    │  Source  │  │
│  │  Device  │    │          │    │          │    │          │  │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘  │
│       ▲               ▲                               │         │
│       │               │ params                        │         │
│    USB Host      BLE Control                     Bluetooth      │
└───────┼───────────────────────────────────────────────┼─────────┘
        │                                               │
        ▼                                               ▼
   ┌─────────┐                                    ┌─────────┐
   │   PC    │                                    │Headphones│
   │ (Audio) │                                    │ (A2DP)  │
   └─────────┘                                    └─────────┘
```

## Quick Start

### Prerequisites

1. **Rust toolchain** with ESP support:
   ```bash
   # Install espup (ESP Rust toolchain manager)
   cargo install espup
   espup install
   
   # Source the environment
   source ~/export-esp.sh
   ```

2. **ESP-IDF** (will be downloaded by esp-idf-sys):
   ```bash
   # Or manually install ESP-IDF 5.2
   git clone --recursive https://github.com/espressif/esp-idf.git
   cd esp-idf && git checkout v5.2
   ./install.sh esp32s3
   source export.sh
   ```

3. **Flash tools**:
   ```bash
   cargo install espflash
   cargo install ldproxy
   ```

### Build & Flash

```bash
# Clone and build
cd esp32-audio-dsp
cargo build --release

# Flash to device
cargo run --release
# Or: espflash flash target/xtensa-esp32s3-espidf/release/esp32-audio-dsp --monitor
```

## Usage

### 1. Connect USB Audio

1. Connect ESP32-S3 to your computer via USB
2. The device appears as "USB Speaker with DSP"
3. Select it as your audio output device

### 2. Pair Bluetooth Headphones

Using the phone app (or serial console):
1. Put headphones in pairing mode
2. Send discovery command via BLE
3. Select your headphones from discovered devices
4. Wait for A2DP connection

### 3. Control Filter Parameters

Via BLE from the phone app:
- Select filter preset (Low-pass, High-pass, Band-pass, Parametric)
- Adjust cutoff frequency, Q factor, gain
- Upload custom FIR coefficients
- Monitor audio levels and latency

## BLE Protocol

### Service UUID
`12345678-1234-5678-1234-56789abcdef0`

### Characteristics

| UUID | Name | Properties | Description |
|------|------|------------|-------------|
| `...def1` | Control | Write | Send commands to device |
| `...def2` | Status | Read/Notify | Get device status |

### Commands (postcard/serde serialized)

```rust
enum BleCommand {
    SetFilterPreset(FilterPreset),
    SetCustomFilter { coefficients: Vec<f32> },
    SetVolume(u8),
    GetStatus,
    StartDiscovery,
    Connect([u8; 6]),
    Disconnect,
}
```

## FIR Filter Presets

| Preset | Description | Parameters |
|--------|-------------|------------|
| Bypass | No filtering | None |
| LowPass | Low-pass filter | `cutoff_hz` |
| HighPass | High-pass filter | `cutoff_hz` |
| BandPass | Band-pass filter | `low_hz`, `high_hz` |
| Parametric | Parametric EQ | `center_hz`, `gain_db`, `q` |
| Custom | Custom coefficients | Up to 128 taps |

## Project Structure

```
esp32-audio-dsp/
├── Cargo.toml              # Dependencies and build config
├── build.rs                # ESP-IDF build integration
├── sdkconfig.defaults      # ESP-IDF configuration
├── partitions.csv          # Flash partition table
├── .cargo/
│   └── config.toml         # Cargo target configuration
└── src/
    ├── main.rs             # Entry point, task coordination
    ├── lib.rs              # Module exports
    ├── fir_filter.rs       # FIR DSP implementation
    ├── usb_audio.rs        # USB Audio Class device
    ├── bluetooth.rs        # A2DP + BLE management
    └── protocol.rs         # BLE communication protocol
```

## Safety & Verification

This project follows embedded Rust best practices for safety-critical audio:

- **No heap in hot path**: Audio processing uses only stack/static allocation
- **Atomic operations**: Thread-safe state management between USB, BLE, and audio tasks
- **Bounded buffers**: All buffers have compile-time size limits
- **Panic = reset**: Unrecoverable errors trigger safe device restart
- **Const generics**: Maximum filter taps enforced at compile time

## Performance

| Metric | Value |
|--------|-------|
| Sample Rate | 48 kHz |
| Bit Depth | 16-bit |
| Channels | Stereo |
| USB Buffer | 4 × 1ms frames |
| FIR Taps | Up to 128 |
| Target Latency | ~50ms |
| CPU Usage | ~30% (240MHz) |

## Troubleshooting

### USB not recognized
- Ensure you're using an ESP32-S3 (not S2, C3, or original ESP32)
- Check USB cable supports data (not charge-only)
- Try different USB port

### No Bluetooth audio
- Verify headphones are in A2DP sink mode
- Check ESP32 is in range
- Try forgetting and re-pairing headphones

### Audio glitches
- Reduce filter complexity (fewer taps)
- Check for WiFi interference
- Ensure adequate power supply

## License

MIT OR Apache-2.0

## Contributing

Contributions welcome! Please ensure:
1. Code compiles for `xtensa-esp32s3-espidf` target
2. No `unsafe` without documented safety invariants
3. Tests pass: `cargo test --lib`
