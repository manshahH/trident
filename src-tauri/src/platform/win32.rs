use std::ffi::c_void;

use windows::Win32::{
    Foundation::{GetLastError, SetLastError, HWND, WIN32_ERROR},
    UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST,
    },
};

use crate::error::{AppError, Result};

pub fn set_no_activate(raw_hwnd: *mut c_void) -> Result<()> {
    let hwnd = HWND(raw_hwnd);
    unsafe {
        SetLastError(WIN32_ERROR(0));
        let current_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        if current_style == 0 {
            let error = GetLastError();
            if error != WIN32_ERROR(0) {
                return Err(platform_error("GetWindowLongPtrW", error));
            }
        }

        let updated_style = current_style
            | WS_EX_NOACTIVATE.0 as isize
            | WS_EX_TOOLWINDOW.0 as isize
            | WS_EX_TOPMOST.0 as isize;
        SetLastError(WIN32_ERROR(0));
        let previous_style = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, updated_style);
        if previous_style == 0 {
            let error = GetLastError();
            if error != WIN32_ERROR(0) {
                return Err(platform_error("SetWindowLongPtrW", error));
            }
        }
    }

    Ok(())
}

fn platform_error(api: &str, error: WIN32_ERROR) -> AppError {
    AppError::Platform {
        api: api.to_owned(),
        code: error.0 as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::set_no_activate;

    #[test]
    fn reports_a_platform_error_for_an_invalid_window_handle() -> crate::error::Result<()> {
        let error = match set_no_activate(std::ptr::null_mut()) {
            Ok(()) => {
                return Err(crate::error::AppError::Internal(
                    "an invalid window handle must be rejected".to_owned(),
                ));
            }
            Err(error) => error,
        };

        assert!(matches!(
            error,
            crate::error::AppError::Platform { ref api, .. } if api == "GetWindowLongPtrW"
        ));
        Ok(())
    }
}
