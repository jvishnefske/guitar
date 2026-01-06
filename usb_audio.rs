//! USB Audio Module
//!
//! Stub implementation for USB Audio Class device.

use heapless::spsc::Queue;

/// Number of audio buffers in the queue
pub const NUM_BUFFERS: usize = 4;

/// Audio buffer size (stereo samples at 48kHz, 10ms)
pub const BUFFER_SIZE: usize = 48000 * 2 * 10 / 1000; // 960 samples

/// Audio buffer for USB transfer
#[derive(Default)]
pub struct AudioBuffer {
    pub samples: [i16; BUFFER_SIZE],
    pub valid_samples: usize,
}

impl AudioBuffer {
    pub fn new() -> Self {
        Self {
            samples: [0; BUFFER_SIZE],
            valid_samples: 0,
        }
    }
}

/// USB Audio Class device
pub struct UsbAudioDevice<'a> {
    queue: &'a mut Queue<AudioBuffer, NUM_BUFFERS>,
    volume: u8,
}

impl<'a> UsbAudioDevice<'a> {
    pub fn new(queue: &'a mut Queue<AudioBuffer, NUM_BUFFERS>) -> Self {
        Self {
            queue,
            volume: 100,
        }
    }

    pub fn init(&mut self) -> Result<(), &'static str> {
        log::info!("USB Audio: Initializing (stub)");
        Ok(())
    }

    pub fn get_audio_buffer(&mut self) -> Option<AudioBuffer> {
        self.queue.dequeue()
    }

    pub fn apply_volume(&self, buffer: &mut AudioBuffer) {
        let scale = self.volume as i32;
        for sample in buffer.samples[..buffer.valid_samples].iter_mut() {
            *sample = ((*sample as i32 * scale) / 100) as i16;
        }
    }

    pub fn set_volume(&mut self, volume: u8) {
        self.volume = volume.min(100);
    }
}
