//! Conventional UTF-8 SHIORI ABI for macOS. Inputs and outputs use malloc/free.
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::ptr;
use std::sync::Mutex;

use crate::actor::lifecycle;

const MAX_BYTES: i32 = 8 * 1024 * 1024;
#[derive(Clone, Copy, PartialEq)]
enum State {
    Empty,
    Loaded,
    Failed,
}
static STATE: Mutex<State> = Mutex::new(State::Empty);

struct Input(*mut c_void);
impl Drop for Input {
    fn drop(&mut self) {
        unsafe { libc::free(self.0) };
    }
}
impl Input {
    unsafe fn text(&self, length: i32) -> Option<&str> {
        if self.0.is_null() || !(1..=MAX_BYTES).contains(&length) {
            return None;
        }
        let bytes = unsafe { std::slice::from_raw_parts(self.0.cast::<u8>(), length as usize) };
        std::str::from_utf8(bytes).ok()
    }
}

/// # Safety
/// `directory` must be malloc-owned and readable for `length` bytes when valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn loadu(directory: *mut c_void, length: i32) -> i32 {
    let input = Input(directory);
    catch_unwind(AssertUnwindSafe(|| {
        let Ok(mut state) = STATE.try_lock() else {
            return 0;
        };
        if *state != State::Empty {
            return 0;
        }
        let Some(text) = (unsafe { input.text(length) }) else {
            return 0;
        };
        let path = PathBuf::from(text);
        if text.contains('\0') || !path.is_absolute() || !path.is_dir() {
            return 0;
        }
        // A panic or failed teardown prevents reusing an uncertain actor state.
        *state = State::Failed;
        if lifecycle::spawn_actor(0, path) {
            *state = State::Loaded;
            1
        } else {
            if lifecycle::teardown_actor().anomaly.is_none() {
                *state = State::Empty;
            }
            0
        }
    }))
    .unwrap_or(0)
}

/// # Safety
/// Same ownership and UTF-8 contract as `loadu`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn load(directory: *mut c_void, length: i32) -> i32 {
    unsafe { loadu(directory, length) }
}

#[unsafe(no_mangle)]
pub extern "C" fn unload() -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        let Ok(mut state) = STATE.try_lock() else {
            return 0;
        };
        match *state {
            State::Empty => 1,
            State::Failed => 0,
            State::Loaded => {
                *state = State::Failed;
                if lifecycle::teardown_actor().anomaly.is_some() {
                    return 0;
                }
                *state = State::Empty;
                1
            }
        }
    }))
    .unwrap_or(0)
}

/// # Safety
/// `message` is malloc-owned; `length`, when non-null, points to a writable i32.
/// The caller frees the returned response with free().
#[unsafe(no_mangle)]
pub unsafe extern "C" fn request(message: *mut c_void, length: *mut i32) -> *mut c_void {
    let input = Input(message);
    if length.is_null() {
        return ptr::null_mut();
    }
    let size = unsafe { *length };
    unsafe { *length = 0 };
    catch_unwind(AssertUnwindSafe(|| {
        let Ok(state) = STATE.try_lock() else {
            return ptr::null_mut();
        };
        if *state != State::Loaded {
            return ptr::null_mut();
        }
        let Some(text) = (unsafe { input.text(size) }) else {
            return ptr::null_mut();
        };
        let response = lifecycle::marshal_request(text);
        if response.is_empty() || response.len() > MAX_BYTES as usize {
            return ptr::null_mut();
        }
        let buffer = unsafe { libc::malloc(response.len()) };
        if buffer.is_null() {
            return ptr::null_mut();
        }
        unsafe {
            ptr::copy_nonoverlapping(response.as_ptr(), buffer.cast::<u8>(), response.len());
            *length = response.len() as i32;
        }
        buffer
    }))
    .unwrap_or(ptr::null_mut())
}
