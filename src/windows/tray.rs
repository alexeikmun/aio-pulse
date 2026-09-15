use tracing::{error, info};
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    LoadIconW, HICON, IDI_APPLICATION,
};

pub const WM_TRAYICON: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;
pub const TRAY_ICON_ID: u32 = 1001;

pub struct TrayIcon {
    hwnd: HWND,
    icon: HICON,
    added: bool,
}

impl TrayIcon {
    pub fn new(hwnd: HWND, tooltip: &str) -> Self {
        unsafe {
            let icon = LoadIconW(None, PCWSTR(IDI_APPLICATION.0 as *const u16))
                .unwrap_or_default();

            let mut tray = Self {
                hwnd,
                icon,
                added: false,
            };

            tray.init(tooltip);
            tray
        }
    }

    fn init(&mut self, tooltip: &str) {
        unsafe {
            let mut nid = NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = TRAY_ICON_ID;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_TRAYICON;
            nid.hIcon = self.icon;

            let tip_utf16: Vec<u16> = tooltip.encode_utf16().take(127).collect();
            for (i, &ch) in tip_utf16.iter().enumerate() {
                nid.szTip[i] = ch;
            }

            if Shell_NotifyIconW(NIM_ADD, &nid).as_bool() {
                self.added = true;
                info!("System tray icon registered successfully.");
            } else {
                error!("Failed to register system tray icon.");
            }
        }
    }

    pub fn update_tip(&self, tooltip: &str) {
        if !self.added {
            return;
        }
        unsafe {
            let mut nid = NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = TRAY_ICON_ID;
            nid.uFlags = NIF_TIP;

            let tip_utf16: Vec<u16> = tooltip.encode_utf16().take(127).collect();
            for (i, &ch) in tip_utf16.iter().enumerate() {
                nid.szTip[i] = ch;
            }

            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    pub fn remove(&mut self) {
        if self.added {
            unsafe {
                let mut nid = NOTIFYICONDATAW::default();
                nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = self.hwnd;
                nid.uID = TRAY_ICON_ID;
                let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
                self.added = false;
                info!("System tray icon removed.");
            }
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        self.remove();
    }
}
