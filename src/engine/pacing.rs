//! **Frame pacing**: VSync and the FPS cap.
//!
//! VSync is asked for when the window opens, but drivers are free to ignore
//! that, and turning it on or off in Options should work straight away. So
//! here the swap interval is set directly on the current OpenGL context
//! (WGL on Windows; GLX or EGL on Linux). On macOS the window is paced by the
//! display itself and there's no turning that off; the cap still works.
//!
//! The FPS cap waits out each frame's share of a second, counted from when the
//! last frame really started, so it's a hard cap: no frame comes sooner, even
//! after a slow one. `sleep` alone can be a long way off (Windows' default
//! timer ticks every 15.6 ms, so a 144 cap came out near 64), so it sleeps
//! most of the way and spins the last bit.
//!
//! Some drivers ignore the swap interval (or are set to override it), so with
//! VSync on the frames are also held to the display's refresh rate: a hair
//! over it, so when VSync does work it's never the wait that's late for a
//! refresh.

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
/// last one started. Never sooner than a whole share after it: a slow frame
/// isn't made up for by rushing the next ones.
pub fn next_due(last: Instant, fps: f32) -> Option<Instant> {
    (fps > 0.0).then(|| last + Duration::from_secs_f64(1.0 / fps as f64))
}

/// The frame rate to hold to: the Max FPS setting (0: none), and with VSync
/// on, the display's refresh rate (if we can find it out) too.
pub fn cap(max_fps: u32, vsync: bool, refresh: Option<u32>) -> f32 {
    let synced = match refresh {
        Some(hz) if vsync && hz > 0 => hz as f32 * 1.02,
        _ => 0.0,
    };
    match (max_fps as f32, synced) {
        (m, 0.0) => m,
        (0.0, s) => s,
        (m, s) => m.min(s),
    }
}

/// The display's refresh rate in Hz, if this platform will tell us.
pub fn refresh_rate() -> Option<u32> {
    imp::refresh_rate().filter(|hz| (20..=1000).contains(hz))
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

    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumDisplaySettingsW(device: *const u16, mode: u32, devmode: *mut u8) -> i32;
    }

    /// The main display's refresh rate (DEVMODEW's dmDisplayFrequency).
    pub fn refresh_rate() -> Option<u32> {
        const ENUM_CURRENT_SETTINGS: u32 = u32::MAX;
        // DEVMODEW is 220 bytes: dmSize at 68, dmDisplayFrequency at 184.
        let mut dm = [0u8; 220];
        dm[68..70].copy_from_slice(&220u16.to_le_bytes());
        let ok = unsafe { EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, dm.as_mut_ptr()) };
        (ok != 0).then(|| u32::from_le_bytes(dm[184..188].try_into().unwrap()))
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

    /// The screen's refresh rate from XRandR (X11, and XWayland).
    pub fn refresh_rate() -> Option<u32> {
        unsafe {
            let x11 = dlopen(c"libX11.so.6".as_ptr(), RTLD_NOW);
            let xrr = dlopen(c"libXrandr.so.2".as_ptr(), RTLD_NOW);
            let (open, root, close) = (sym(x11, c"XOpenDisplay"), sym(x11, c"XDefaultRootWindow"), sym(x11, c"XCloseDisplay"));
            let (info, rate, free) = (sym(xrr, c"XRRGetScreenInfo"), sym(xrr, c"XRRConfigCurrentRate"), sym(xrr, c"XRRFreeScreenConfigInfo"));
            if [open, root, close, info, rate, free].iter().any(|p| p.is_null()) {
                return None;
            }
            let open: extern "C" fn(*const c_char) -> *mut c_void = std::mem::transmute(open);
            let root: extern "C" fn(*mut c_void) -> std::ffi::c_ulong = std::mem::transmute(root);
            let close: extern "C" fn(*mut c_void) -> c_int = std::mem::transmute(close);
            let info: extern "C" fn(*mut c_void, std::ffi::c_ulong) -> *mut c_void = std::mem::transmute(info);
            let rate: extern "C" fn(*mut c_void) -> std::ffi::c_short = std::mem::transmute(rate);
            let free: extern "C" fn(*mut c_void) = std::mem::transmute(free);
            let d = open(std::ptr::null());
            if d.is_null() {
                return None;
            }
            let conf = info(d, root(d));
            let hz = if conf.is_null() {
                None
            } else {
                let hz = rate(conf);
                free(conf);
                Some(hz.max(0) as u32)
            };
            close(d);
            hz
        }
    }
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos"), not(target_os = "android")))))]
mod imp {
    pub fn set_swap_interval(_n: i32) -> bool {
        false
    }

    pub fn refresh_rate() -> Option<u32> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cap_paces_frames_and_doesnt_rush_after_a_hitch() {
        let t0 = Instant::now();
        assert_eq!(next_due(t0, 0.0), None);
        let due = next_due(t0, 100.0).unwrap();
        assert_eq!(due - t0, Duration::from_millis(10));
        // VSync holds frames to the refresh rate too, whichever is lower.
        assert_eq!(cap(0, false, Some(60)), 0.0, "no cap at all");
        assert_eq!(cap(144, false, Some(60)), 144.0);
        assert!((cap(0, true, Some(60)) - 61.2).abs() < 0.01, "a hair over the refresh rate");
        assert_eq!(cap(30, true, Some(60)), 30.0);
        assert_eq!(cap(0, true, None), 0.0, "unknown refresh rate: the driver's VSync alone");
        // Waiting lands close to the time.
        let target = Instant::now() + Duration::from_millis(5);
        wait_until(target);
        let over = Instant::now() - target;
        // (Windows sleeps more coarsely, and a busy test machine can be a few
        // milliseconds late waking us; well under a frame either way.)
        let slack = if cfg!(windows) { Duration::from_millis(12) } else { Duration::from_millis(3) };
        assert!(over < slack, "{over:?}");
    }
}
