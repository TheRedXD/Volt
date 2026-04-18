use parking_lot::Mutex;
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};
use std::{num::NonZeroUsize, sync::Arc};

pub struct AudioBuffer<T> {
    producer: Arc<Mutex<HeapProd<T>>>,
    consumer: Arc<Mutex<HeapCons<T>>>,
    capacity: usize,
    channels: NonZeroUsize,
}

impl<T: Copy + Default> AudioBuffer<T> {
    /// Create a new audio buffer.
    /// - `capacity`: Number of samples per channel
    /// - `channel`: Number of channels for this buffer; for example, 1 for mono, 2 for stereo
    #[must_use]
    pub fn new(capacity: usize, channels: NonZeroUsize) -> Self {
        let (producer, consumer) = HeapRb::new(capacity * channels.get()).split();
        Self {
            producer: Arc::new(Mutex::new(producer)),
            consumer: Arc::new(Mutex::new(consumer)),
            capacity,
            channels,
        }
    }

    /// Write audio data to the buffer and return the number of complete frames written.
    /// Ensures only complete frames are written to prevent audio artifacts.
    pub fn write_frames(&self, data: &[T]) -> usize {
        if data.is_empty() {
            return 0;
        }

        let written = {
            let mut producer = self.producer.lock();
            let available_samples = producer.vacant_len();
            let available_frames = available_samples / self.channels();
            let input_frames = data.len() / self.channels();

            // Only write complete frames
            let frames_to_write = available_frames.min(input_frames);
            let samples_to_write = frames_to_write * self.channels();

            if samples_to_write == 0 {
                return 0;
            }

            producer.push_slice(&data[..samples_to_write])
        };

        written / self.channels() // Return frames
    }

    /// Read audio data from the buffer
    /// Returns the number of complete frames read
    /// Ensures only complete frames are read to prevent audio artifacts
    pub fn read_frames(&self, data: &mut [T]) -> usize {
        if data.is_empty() {
            return 0;
        }

        let read = {
            let mut consumer = self.consumer.lock();
            let available_samples = consumer.occupied_len();
            let available_frames = available_samples / self.channels();
            let output_frames = data.len() / self.channels();

            // Only read complete frames
            let frames_to_read = available_frames.min(output_frames);
            let samples_to_read = frames_to_read * self.channels();

            if samples_to_read == 0 {
                return 0;
            }

            consumer.pop_slice(&mut data[..samples_to_read])
        };

        read / self.channels()
    }

    pub fn clear(&self) {
        let mut consumer = self.consumer.lock();
        consumer.clear();
    }

    /// Get number of available frames
    #[must_use]
    pub fn len_frames(&self) -> usize {
        let consumer = self.consumer.lock();
        consumer.occupied_len() / self.channels()
    }

    /// Get number of available samples (for compatibility)
    #[must_use]
    pub fn len(&self) -> usize {
        let consumer = self.consumer.lock();
        consumer.occupied_len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        let consumer = self.consumer.lock();
        consumer.occupied_len() == 0
    }

    /// Get available space in frames
    #[must_use]
    pub fn available_frames(&self) -> usize {
        let producer = self.producer.lock();
        producer.vacant_len() / self.channels()
    }

    /// Get available space in samples
    #[must_use]
    pub fn available_space(&self) -> usize {
        let producer = self.producer.lock();
        producer.vacant_len()
    }

    #[must_use]
    /// Total number of samples; capacity per channel multiplied by number of channels
    pub const fn capacity(&self) -> usize {
        self.channel_capacity() * self.channels()
    }

    #[must_use]
    /// Number of samples per channel
    pub const fn channel_capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub const fn channels(&self) -> usize {
        self.channels.get()
    }

    /// Check if buffer has enough space for complete frames
    #[must_use]
    pub fn can_write_frames(&self, frame_count: usize) -> bool {
        self.available_frames() >= frame_count
    }

    /// Check if buffer has enough data for complete frames
    #[must_use]
    pub fn can_read_frames(&self, frame_count: usize) -> bool {
        self.len_frames() >= frame_count
    }
}

pub type SampleBuffer = AudioBuffer<f32>;
