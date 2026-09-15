use windows::Win32::Foundation::HWND;
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use tracing::{info, warn};

pub struct DpiContext;

impl DpiContext {
    pub fn enable_dpi_awareness() {
        unsafe {
            if let Err(err) = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) {
                warn!("SetProcessDpiAwarenessContext failed: {}. Continuing with system default.", err);
            } else {
                info!("Per-Monitor V2 DPI awareness enabled.");
            }
        }
    }

    pub fn get_for_window(hwnd: HWND) -> u32 {
        unsafe {
            let dpi = GetDpiForWindow(hwnd);
            if dpi == 0 {
                96
            } else {
                dpi
            }
        }
    }

    pub fn scale(val: f32, dpi: u32) -> f32 {
        val * (dpi as f32 / 96.0)
    }
}
