/// Result type for rack operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur when working with audio plugins
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// AudioUnit-specific error (OSStatus code)
    #[error("AudioUnit error: {0} (OSStatus)")]
    AudioUnit(i32),

    /// Plugin not found during scanning
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),

    /// Invalid parameter index
    #[error("Invalid parameter index: {0}")]
    InvalidParameter(usize),

    /// Plugin not initialized
    #[error("Plugin not initialized")]
    NotInitialized,

    /// Invalid plugin format
    #[error("Invalid plugin format: {0}")]
    InvalidFormat(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Create an AudioUnit error from an OSStatus code
    pub fn from_os_status(status: i32) -> Self {
        Error::AudioUnit(status)
    }
}
