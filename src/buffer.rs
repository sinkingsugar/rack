//! Aligned audio buffers for SIMD processing

use aligned_vec::{AVec, ConstAlign};
use std::ops::{Deref, DerefMut};

/// A 16-byte aligned audio buffer for optimal SIMD performance
///
/// This buffer ensures data is aligned to 16-byte boundaries on the heap,
/// enabling faster aligned SIMD operations (SSE2, NEON) compared to regular Vec<f32>.
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
pub struct AudioBuffer {
    data: AVec<f32, ConstAlign<16>>,
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
            data: AVec::from_iter(16, std::iter::repeat(0.0).take(size)),
        }
    }

    /// Creates a new aligned audio buffer from existing data
    ///
    /// Note: The data will be copied to ensure proper 16-byte alignment.
    ///
    /// # Examples
    ///
    /// ```
    /// use rack::AudioBuffer;
    ///
    /// let data = vec![1.0, 2.0, 3.0, 4.0];
    /// let buffer = AudioBuffer::from_slice(&data);
    /// assert_eq!(buffer.len(), 4);
    /// ```
    #[inline]
    pub fn from_slice(data: &[f32]) -> Self {
        Self {
            data: AVec::from_iter(16, data.iter().copied()),
        }
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
    fn test_alignment_after_operations() {
        // Test that alignment is preserved after various operations
        let mut buffer = AudioBuffer::new(100);
        assert_eq!(buffer.as_ptr() as usize % 16, 0);

        buffer.resize(200);
        assert_eq!(buffer.as_ptr() as usize % 16, 0);

        buffer.clear();
        assert_eq!(buffer.as_ptr() as usize % 16, 0);

        let cloned = buffer.clone();
        assert_eq!(cloned.as_ptr() as usize % 16, 0);
    }

    #[test]
    fn test_new() {
        let buffer = AudioBuffer::new(100);
        assert_eq!(buffer.len(), 100);
        assert!(buffer.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_from_slice() {
        let data = vec![1.0, 2.0, 3.0];
        let buffer = AudioBuffer::from_slice(&data);
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer[0], 1.0);
        assert_eq!(buffer[1], 2.0);
        assert_eq!(buffer[2], 3.0);
        // Verify alignment
        assert_eq!(buffer.as_ptr() as usize % 16, 0);
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
        let mut buffer = AudioBuffer::from_slice(&[1.0, 2.0, 3.0]);
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
