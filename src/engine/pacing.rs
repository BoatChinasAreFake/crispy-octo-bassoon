//! **Frame pacing**: VSync and the FPS cap.
//!
//! VSync is asked for when the window opens, but drivers are free to ignore
//! that, and turning it on or off in Options should work straight away. So
//! here the swap interval is set directly on the current OpenGL context
//! (WGL on Windows; GLX or EGL on Linux). On macOS the window is paced by the
//! display itself and there's no turning that off; the cap still works.
//!
//! The FPS cap waits out each frame's share of a second. `sleep` alone can be
//! a long way off (Windows' default timer ticks every 15.6 ms, so a 144 cap
//! came out near 64), so it sleeps most of the way and spins the last bit.

use std::time::{Duration, Instant};

/// Ask the timer for millisecond sleeps (Windows; elsewhere they already are).
pub fn init() {
    #[cfg(windows)]
    unsafe {
        #[link(name = "winmm")]
        unsafe extern "system" {
            fn timeBeginPeriod(period: u32) -> u32;
        }
        timeBeginPeriod(1);
    }
}

/// Wait until `due`, as close to it as we can manage.
pub fn wait_until(due: Instant) {
    // Sleep while there's plenty of time, then spin (yielding) for the rest.
    const SPIN: Duration = Duration::from_micros(1500);
    loop {
        let now = Instant::now();
        if now >= due {
            return;
        }
        let left = due - now;
        if left > SPIN {
            std::thread::sleep(left - SPIN);
        } else {
            std::thread::yield_now();
        }
    }
}

/// When the next frame is due under a cap of `fps` (0: no cap), given when the
/// last one was due. Falls back to now if we've slipped more than a frame behind
/// (so a hitch doesn't make the next frames rush to catch up).
pub fn next_due(last: Instant, fps: u32, now: Instant) -> Option<Instant> {
    if fps == 0 {
        return None;
    }
    let share = Duration::from_secs_f64(1.0 / fps as f64);
    let due = last + share;
    Some(if due + share < now { now } else { due })
}

/// Turn VSync on or off on the current context. False if this platform or
/// driver gave us no way to (it then keeps whatever the window opened with).
pub fn set_vsync(on: bool) -> bool {
    imp::set_swap_interval(on as i32)
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    #[link(name = "opengl32")]
    unsafe extern "system" {
        fn wglGetProcAddress(name: *const std::ffi::c_char) -> *const c_void;
    }
    pub fn set_swap_interval(n: i32) -> bool {
        unsafe {
            let f = wglGetProcAddress(c"wglSwapIntervalEXT".as_ptr());
            if f.is_null() {
                return false;
            }
            let f: extern "system" fn(i32) -> i32 = std::mem::transmute(f);
            f(n) != 0
        }
    }
}

#[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
mod imp {
    use std::ffi::{c_char, c_int, c_void};
    #[link(name = "dl")]
    unsafe extern "C" {
        fn dlopen(file: *const c_char, mode: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    }
    const RTLD_NOW: c_int = 2;

    unsafe fn sym(lib: *mut c_void, name: &std::ffi::CStr) -> *mut c_void {
        if lib.is_null() { std::ptr::null_mut() } else { unsafe { dlsym(lib, name.as_ptr()) } }
    }

    pub fn set_swap_interval(n: i32) -> bool {
        unsafe {
            // GLX (what the window usually gets on X11)...
            let gl = dlopen(c"libGL.so.1".as_ptr(), RTLD_NOW);
            let display = sym(gl, c"glXGetCurrentDisplay");
            let drawable = sym(gl, c"glXGetCurrentDrawable");
            if !display.is_null() && !drawable.is_null() {
                let display: extern "C" fn() -> *mut c_void = std::mem::transmute(display);
                let drawable: extern "C" fn() -> std::ffi::c_ulong = std::mem::transmute(drawable);
                let (d, w) = (display(), drawable());
                if !d.is_null() && w != 0 {
                    let ext = sym(gl, c"glXSwapIntervalEXT");
                    if !ext.is_null() {
                        let f: extern "C" fn(*mut c_void, std::ffi::c_ulong, c_int) = std::mem::transmute(ext);
                        f(d, w, n);
                        return true;
                    }
                    let mesa = sym(gl, c"glXSwapIntervalMESA");
                    if !mesa.is_null() {
                        let f: extern "C" fn(c_int) -> c_int = std::mem::transmute(mesa);
                        return f(n) == 0;
                    }
                }
            }
            // ...or EGL (Wayland, and X11 when GLX isn't there).
            let egl = dlopen(c"libEGL.so.1".as_ptr(), RTLD_NOW);
            let current = sym(egl, c"eglGetCurrentDisplay");
            let interval = sym(egl, c"eglSwapInterval");
            if current.is_null() || interval.is_null() {
                return false;
            }
            let current: extern "C" fn() -> *mut c_void = std::mem::transmute(current);
            let interval: extern "C" fn(*mut c_void, c_int) -> c_int = std::mem::transmute(interval);
            let d = current();
            !d.is_null() && interval(d, n) != 0
        }
    }
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos"), not(target_os = "android")))))]
mod imp {
    pub fn set_swap_interval(_n: i32) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cap_paces_frames_and_doesnt_rush_after_a_hitch() {
        let t0 = Instant::now();
        assert_eq!(next_due(t0, 0, t0), None);
        let due = next_due(t0, 100, t0).unwrap();
        assert_eq!(due - t0, Duration::from_millis(10));
        // Way behind (a hitch): start again from now rather than rushing.
        let late = t0 + Duration::from_millis(500);
        assert_eq!(next_due(t0, 100, late), Some(late));
        // Waiting lands close to the time.
        let target = Instant::now() + Duration::from_millis(5);
        wait_until(target);
        let over = Instant::now() - target;
        assert!(over < Duration::from_millis(3), "{over:?}");
    }
}
