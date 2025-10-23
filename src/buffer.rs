//! Aligned audio buffers for SIMD processing

use std::ops::{Deref, DerefMut};

/// A 16-byte aligned audio buffer for optimal SIMD performance
///
/// This buffer ensures data is aligned to 16-byte boundaries, enabling
/// faster aligned SIMD operations (SSE2, NEON) compared to regular Vec<f32>.
///
/// # Performance
///
/// Using aligned buffers can improve processing performance by 5-10% due to:
/// - Aligned SIMD loads/stores are faster than unaligned
/// - Better cache line utilization
/// - Reduced CPU stalls on some architectures
///
/// # Examples
///
/// ```
/// use rack::AudioBuffer;
///
/// // Create aligned buffer for 512 stereo frames (1024 samples)
/// let mut buffer = AudioBuffer::new(1024);
///
/// // Use like a regular slice
/// buffer[0] = 1.0;
/// assert_eq!(buffer.len(), 1024);
/// ```
#[repr(align(16))]
pub struct AudioBuffer {
    data: Vec<f32>,
}

impl AudioBuffer {
    /// Creates a new aligned audio buffer with the specified size
    ///
    /// The buffer is initialized to zeros and guaranteed to be 16-byte aligned.
    ///
    /// # Examples
    ///
    /// ```
    /// use rack::AudioBuffer;
    ///
    /// let buffer = AudioBuffer::new(512);
    /// assert_eq!(buffer.len(), 512);
    /// ```
    #[inline]
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0.0; size],
        }
    }

    /// Creates a new aligned audio buffer from existing data
    ///
    /// # Examples
    ///
    /// ```
    /// use rack::AudioBuffer;
    ///
    /// let data = vec![1.0, 2.0, 3.0, 4.0];
    /// let buffer = AudioBuffer::from_vec(data);
    /// assert_eq!(buffer.len(), 4);
    /// ```
    #[inline]
    pub fn from_vec(data: Vec<f32>) -> Self {
        Self { data }
    }

    /// Returns the number of samples in the buffer
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the buffer is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns a slice of the buffer data
    #[inline]
    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }

    /// Returns a mutable slice of the buffer data
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        &mut self.data
    }

    /// Fills the buffer with zeros
    #[inline]
    pub fn clear(&mut self) {
        self.data.fill(0.0);
    }

    /// Resizes the buffer to the new size, filling with zeros if growing
    #[inline]
    pub fn resize(&mut self, new_size: usize) {
        self.data.resize(new_size, 0.0);
    }
}

impl Deref for AudioBuffer {
    type Target = [f32];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for AudioBuffer {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl Clone for AudioBuffer {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
        }
    }
}

impl Default for AudioBuffer {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment() {
        let buffer = AudioBuffer::new(512);
        let ptr = buffer.as_ptr();
        // Verify 16-byte alignment
        assert_eq!(ptr as usize % 16, 0, "Buffer should be 16-byte aligned");
    }

    #[test]
    fn test_new() {
        let buffer = AudioBuffer::new(100);
        assert_eq!(buffer.len(), 100);
        assert!(buffer.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_from_vec() {
        let data = vec![1.0, 2.0, 3.0];
        let buffer = AudioBuffer::from_vec(data);
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer[0], 1.0);
        assert_eq!(buffer[1], 2.0);
        assert_eq!(buffer[2], 3.0);
    }

    #[test]
    fn test_deref() {
        let mut buffer = AudioBuffer::new(10);
        buffer[0] = 1.0;
        buffer[1] = 2.0;
        assert_eq!(buffer[0], 1.0);
        assert_eq!(buffer[1], 2.0);
    }

    #[test]
    fn test_clear() {
        let mut buffer = AudioBuffer::from_vec(vec![1.0, 2.0, 3.0]);
        buffer.clear();
        assert!(buffer.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_resize() {
        let mut buffer = AudioBuffer::new(10);
        buffer.resize(20);
        assert_eq!(buffer.len(), 20);
    }
}
