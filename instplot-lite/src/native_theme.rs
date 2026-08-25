#[cfg(target_os = "windows")]
pub fn force_dark_title_bar(window_title: &str) {
    use std::{ffi::c_void, iter, mem, ptr};
    use windows_sys::Win32::{
        Graphics::Dwm::DwmSetWindowAttribute,
        UI::WindowsAndMessaging::{
            FindWindowW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos,
        },
    };

    let title = window_title
        .encode_utf16()
        .chain(iter::once(0))
        .collect::<Vec<_>>();

    // SAFETY: `title` is a live, NUL-terminated UTF-16 buffer. The returned
    // handle is checked before it is passed to the Windows APIs below.
    let window = unsafe { FindWindowW(ptr::null(), title.as_ptr()) };
    if window.is_null() {
        return;
    }

    let enabled = 1_i32;
    let value = (&enabled as *const i32).cast::<c_void>();
    let value_size = mem::size_of_val(&enabled) as u32;

    // Windows 10 20H1+ uses attribute 20; older supported Windows 10 builds
    // used 19. Applying both in fallback order keeps the title bar dark without
    // following the user's light/dark theme preference.
    // SAFETY: `window` is valid and `value` points to a correctly sized BOOL.
    let result = unsafe { DwmSetWindowAttribute(window, 20, value, value_size) };
    if result < 0 {
        // SAFETY: Same valid arguments as the call above, for the legacy ID.
        unsafe {
            DwmSetWindowAttribute(window, 19, value, value_size);
        }
    }

    // Force Windows to repaint the non-client area now instead of waiting for
    // an activation or resize event.
    // SAFETY: Flags guarantee that position, size, and z-order arguments are ignored.
    unsafe {
        SetWindowPos(
            window,
            ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub fn force_dark_title_bar(_window_title: &str) {}
