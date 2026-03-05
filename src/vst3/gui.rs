use super::ffi;
use crate::{Error, Result};
use std::ffi::CString;
use std::marker::PhantomData;

/// A VST3 plugin GUI editor handle.
///
/// Owns the underlying `RackVST3Gui` C object. `show_window()` and
/// `hide_window()` must be called from the thread that owns the Win32
/// message pump (typically the main/UI thread).
///
/// # Thread Safety
///
/// `Send` but NOT `Sync` — the handle can be moved between threads, but
/// Win32 window operations must happen on the window-owning thread.
pub struct Vst3Gui {
    inner: *mut ffi::RackVST3Gui,
    _not_sync: PhantomData<*const ()>,
}

// Safety: The C object can be transferred between threads. Window operations
// are only valid on the thread that created the window, which is enforced by
// the caller (Tauri's run_on_main_thread).
unsafe impl Send for Vst3Gui {}

impl Vst3Gui {
    /// Wrap a raw pointer returned by `rack_vst3_gui_create`.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid, non-null pointer from `rack_vst3_gui_create`.
    pub(crate) unsafe fn from_raw(ptr: *mut ffi::RackVST3Gui) -> Self {
        Self {
            inner: ptr,
            _not_sync: PhantomData,
        }
    }

    /// Get the initial/current editor size in pixels.
    pub fn get_size(&self) -> (i32, i32) {
        let mut w: i32 = 0;
        let mut h: i32 = 0;
        unsafe {
            ffi::rack_vst3_gui_get_size(self.inner, &mut w, &mut h);
        }
        (w, h)
    }

    /// Create and show the native editor window.
    ///
    /// **Must be called from the main/UI thread** (the thread running the
    /// Win32 message pump). Tauri's `app.run_on_main_thread()` satisfies this.
    pub fn show_window(&self, title: &str) -> Result<()> {
        let c_title = CString::new(title)
            .map_err(|_| Error::Other("Title contains null byte".to_string()))?;
        unsafe {
            let rc = ffi::rack_vst3_gui_show_window(self.inner, c_title.as_ptr());
            if rc != ffi::RACK_VST3_OK {
                return Err(Error::Other("Failed to show editor window".to_string()));
            }
        }
        Ok(())
    }

    /// Hide (but don't destroy) the editor window.
    ///
    /// **Must be called from the main/UI thread.**
    pub fn hide_window(&self) -> Result<()> {
        unsafe {
            let rc = ffi::rack_vst3_gui_hide_window(self.inner);
            if rc != ffi::RACK_VST3_OK {
                return Err(Error::Other("Failed to hide editor window".to_string()));
            }
        }
        Ok(())
    }

    /// Register a callback invoked when the user closes the editor window
    /// via the X button. The callback fires on the main/UI thread.
    ///
    /// Only one callback can be active at a time; calling this again replaces
    /// the previous one.
    pub fn set_close_callback<F: FnOnce() + Send + 'static>(&self, f: F) {
        // Box the closure, leak it to a raw pointer, pass through the C
        // trampoline, and reconstruct + call it on the other side.
        let boxed: Box<Box<dyn FnOnce() + Send>> = Box::new(Box::new(f));
        let raw = Box::into_raw(boxed) as *mut std::ffi::c_void;

        extern "C" fn trampoline(user_data: *mut std::ffi::c_void) {
            unsafe {
                let boxed: Box<Box<dyn FnOnce() + Send>> =
                    Box::from_raw(user_data as *mut Box<dyn FnOnce() + Send>);
                boxed();
            }
        }

        unsafe {
            ffi::rack_vst3_gui_set_close_callback(self.inner, trampoline, raw);
        }
    }
}

impl Drop for Vst3Gui {
    fn drop(&mut self) {
        unsafe {
            ffi::rack_vst3_gui_destroy(self.inner);
        }
    }
}
