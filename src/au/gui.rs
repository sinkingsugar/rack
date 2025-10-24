//! Safe wrapper for AudioUnit GUI functionality
//!
//! This module provides safe, idiomatic Rust API for creating and managing
//! AudioUnit plugin GUIs on macOS.
//!
//! # Thread Safety
//!
//! **IMPORTANT**: All GUI operations must be called from the main thread.
//! This is a macOS/AppKit requirement. The type system cannot enforce this,
//! so it is the caller's responsibility.
//!
//! # Example
//!
//! ```no_run
//! use rack::prelude::*;
//! use std::sync::{Arc, Mutex};
//!
//! # fn main() -> Result<()> {
//! // Create and initialize plugin
//! let scanner = Scanner::new()?;
//! let plugins = scanner.scan()?;
//! let mut plugin = scanner.load(&plugins[0])?;
//! plugin.initialize(48000.0, 512)?;
//!
//! // Create GUI asynchronously (must be on main thread)
//! let gui = Arc::new(Mutex::new(None));
//! let gui_clone = gui.clone();
//!
//! plugin.create_gui(move |result| {
//!     match result {
//!         Ok(audio_gui) => {
//!             println!("GUI created successfully!");
//!             audio_gui.show_window(Some("My Plugin"))?;
//!             *gui_clone.lock().unwrap() = Some(audio_gui);
//!             Ok(())
//!         }
//!         Err(e) => {
//!             eprintln!("GUI creation failed: {}", e);
//!             Err(e)
//!         }
//!     }
//! });
//!
//! # Ok(())
//! # }
//! ```

use crate::au::ffi;
use crate::error::{Error, Result};
use std::ffi::{c_void, CString};
use std::marker::PhantomData;

/// AudioUnit GUI handle
///
/// Represents a plugin's graphical user interface. The GUI can be embedded
/// in a host application using [`get_native_view()`](AudioUnitGui::get_native_view)
/// or displayed in a standalone window using [`show_window()`](AudioUnitGui::show_window).
///
/// # Thread Safety
///
/// **All methods must be called from the main thread.** This is a macOS/AppKit
/// requirement that cannot be enforced by the type system.
///
/// The type is `Send` but not `Sync` - it can be transferred between threads
/// but must not be accessed concurrently.
///
/// # Lifecycle
///
/// The GUI is automatically destroyed when this struct is dropped. Make sure
/// to keep the `AudioUnitGui` alive as long as you need the GUI displayed.
pub struct AudioUnitGui {
    handle: *mut ffi::RackAUGui,
    _marker: PhantomData<*mut ()>, // !Send + !Sync
}

// Safety: AudioUnitGui can be sent between threads (transferred ownership)
// but must not be accessed concurrently (not Sync)
unsafe impl Send for AudioUnitGui {}

impl AudioUnitGui {
    /// Create AudioUnitGui from raw FFI handle
    ///
    /// # Safety
    ///
    /// - `handle` must be a valid pointer returned from `rack_au_gui_create_async`
    /// - Caller must ensure handle is not used elsewhere
    pub(crate) unsafe fn from_raw(handle: *mut ffi::RackAUGui) -> Self {
        AudioUnitGui {
            handle,
            _marker: PhantomData,
        }
    }

    /// Get native NSView pointer for embedding in host UI
    ///
    /// Returns a raw pointer that can be cast to `NSView*` in Objective-C/Swift code.
    /// The view remains owned by this `AudioUnitGui` and will be destroyed when
    /// this struct is dropped.
    ///
    /// # Returns
    ///
    /// - `Some(ptr)` with NSView pointer (as `*mut c_void`)
    /// - `None` if the GUI is invalid
    ///
    /// # Thread Safety
    ///
    /// Must be called from the main thread.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rack::prelude::*;
    /// # fn example(gui: &AudioUnitGui) {
    /// if let Some(view_ptr) = gui.get_native_view() {
    ///     // view_ptr can be cast to NSView* in Objective-C code
    ///     println!("NSView pointer: {:?}", view_ptr);
    /// }
    /// # }
    /// ```
    pub fn get_native_view(&self) -> Option<*mut c_void> {
        let ptr = unsafe { ffi::rack_au_gui_get_view(self.handle) };
        if ptr.is_null() {
            None
        } else {
            Some(ptr)
        }
    }

    /// Get the size of the GUI view in points
    ///
    /// Returns the current size of the plugin's GUI view.
    ///
    /// # Thread Safety
    ///
    /// Can be called from any thread.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rack::prelude::*;
    /// # fn example(gui: &AudioUnitGui) -> Result<()> {
    /// let (width, height) = gui.get_size()?;
    /// println!("GUI size: {}x{} points", width, height);
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_size(&self) -> Result<(f32, f32)> {
        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;

        let result = unsafe { ffi::rack_au_gui_get_size(self.handle, &mut width, &mut height) };

        if result != ffi::RACK_AU_OK {
            return Err(Error::from_os_status(result));
        }

        Ok((width, height))
    }

    /// Create and show a window containing the plugin GUI
    ///
    /// Creates an NSWindow and displays the plugin's GUI in it. The window
    /// will remain open until closed by the user or until this `AudioUnitGui`
    /// is dropped.
    ///
    /// # Parameters
    ///
    /// - `title`: Optional window title. If `None`, defaults to "AudioUnit GUI"
    ///
    /// # Thread Safety
    ///
    /// Must be called from the main thread.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rack::prelude::*;
    /// # fn example(gui: &AudioUnitGui) -> Result<()> {
    /// // Show with custom title
    /// gui.show_window(Some("My Awesome Plugin"))?;
    ///
    /// // Show with default title
    /// gui.show_window(None)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn show_window(&self, title: Option<&str>) -> Result<()> {
        let c_title = title.map(|t| CString::new(t).unwrap());
        let title_ptr = c_title
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(std::ptr::null());

        let result = unsafe { ffi::rack_au_gui_show_window(self.handle, title_ptr) };

        if result != ffi::RACK_AU_OK {
            return Err(Error::from_os_status(result));
        }

        Ok(())
    }

    /// Hide the window without destroying the GUI
    ///
    /// Hides the window created by [`show_window()`](AudioUnitGui::show_window).
    /// The GUI remains valid and can be shown again.
    ///
    /// # Thread Safety
    ///
    /// Must be called from the main thread.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rack::prelude::*;
    /// # fn example(gui: &AudioUnitGui) -> Result<()> {
    /// gui.show_window(Some("My Plugin"))?;
    /// // ... do some work ...
    /// gui.hide_window()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn hide_window(&self) -> Result<()> {
        let result = unsafe { ffi::rack_au_gui_hide_window(self.handle) };

        if result != ffi::RACK_AU_OK {
            return Err(Error::from_os_status(result));
        }

        Ok(())
    }
}

impl Drop for AudioUnitGui {
    fn drop(&mut self) {
        // Safety: handle is valid until drop, and destroy handles NULL safely
        unsafe {
            ffi::rack_au_gui_destroy(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_unit_gui_is_send() {
        // Compile-time check that AudioUnitGui is Send
        fn assert_send<T: Send>() {}
        assert_send::<AudioUnitGui>();
    }

    #[test]
    fn test_audio_unit_gui_is_not_sync() {
        // Compile-time check that AudioUnitGui is NOT Sync
        fn assert_not_sync<T: Send>() {}
        assert_not_sync::<AudioUnitGui>();

        // This should NOT compile (uncomment to verify):
        // fn assert_sync<T: Sync>() {}
        // assert_sync::<AudioUnitGui>();
    }
}
