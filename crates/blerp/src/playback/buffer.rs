use crate::utils::Channel;
use parking_lot::Mutex;
use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapCons, HeapProd, HeapRb,
};
use std::sync::Arc;

pub struct AudioBuffer<T> {
    producer: Arc<Mutex<HeapProd<T>>>,
    consumer: Arc<Mutex<HeapCons<T>>>,
    capacity: usize,
    channel: Channel,
}

impl<T: Copy + Default> AudioBuffer<T> {
    /// Create a new audio buffer
    /// - capacity: Total number of samples (must be divisible by channels for frame alignment)
    /// - channels: Number of audio channels (1=mono, 2=stereo, etc.)
    pub fn new(capacity: usize, channel: Channel) -> Self {
        assert!(usize::from(channel) > 0, "Must have at least 1 channel");
        assert_eq!(capacity % usize::from(channel), 0, "Capacity must be divisible by channel count for frame alignment");

        let buffer = HeapRb::new(capacity);
        let (producer, consumer) = buffer.split();

        Self {
            producer: Arc::new(Mutex::new(producer)),
            consumer: Arc::new(Mutex::new(consumer)),
            capacity,
            channel,
        }
    }

    /// Write audio data to the buffer
    /// Returns the number of complete frames written
    /// Ensures only complete frames are written to prevent audio artifacts
    pub fn write_frames(&self, data: &[T]) -> usize {
        if data.is_empty() {
            return 0;
        }

        let written = {
            let mut producer = self.producer.lock();
            let available_samples = producer.vacant_len();
            let available_frames = available_samples / self.channels;
            let input_frames = data.len() / self.channels;

            // Only write complete frames
            let frames_to_write = available_frames.min(input_frames);
            let samples_to_write = frames_to_write * self.channels;

            if samples_to_write == 0 {
                return 0;
            }

            producer.push_slice(&data[..samples_to_write])
        };

        written / self.channels // Return frames
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
            let available_frames = available_samples / self.channels;
            let output_frames = data.len() / self.channels;

            // Only read complete frames
            let frames_to_read = available_frames.min(output_frames);
            let samples_to_read = frames_to_read * self.channels;

            if samples_to_read == 0 {
                return 0;
            }

            consumer.pop_slice(&mut data[..samples_to_read])
        };

        read / self.channels
    }

    pub fn clear(&self) {
        let mut consumer = self.consumer.lock();
        consumer.clear();
    }

    /// Get number of available frames
    pub fn len_frames(&self) -> usize {
        let consumer = self.consumer.lock();
        consumer.occupied_len() / self.channels
    }

    /// Get number of available samples (for compatibility)
    pub fn len(&self) -> usize {
        let consumer = self.consumer.lock();
        consumer.occupied_len()
    }

    pub fn is_empty(&self) -> bool {
        let consumer = self.consumer.lock();
        consumer.occupied_len() == 0
    }

    /// Get available space in frames
    pub fn available_frames(&self) -> usize {
        let producer = self.producer.lock();
        producer.vacant_len() / self.channels
    }

    /// Get available space in samples
    pub fn available_space(&self) -> usize {
        let producer = self.producer.lock();
        producer.vacant_len()
    }

    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    pub const fn channels(&self) -> usize {
        self.channels
    }

    /// Check if buffer has enough space for complete frames
    pub fn can_write_frames(&self, frame_count: usize) -> bool {
        self.available_frames() >= frame_count
    }

    /// Check if buffer has enough data for complete frames
    pub fn can_read_frames(&self, frame_count: usize) -> bool {
        self.len_frames() >= frame_count
    }
}
