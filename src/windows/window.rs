use std::sync::Arc;
use tracing::{error, info};
use windows::core::{w, BOOL, PCWSTR, Result};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, UpdateWindow, PAINTSTRUCT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForSystem};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::dpi::DpiContext;
use super::tray::{TrayIcon, WM_TRAYICON};
use crate::app::application::{Application, WM_APP_UPDATE};
use crate::hardware::device::DeviceConnectionStatus;
use crate::rendering::direct2d::Direct2DContext;
use crate::rendering::directwrite::DirectWriteContext;
use crate::rendering::lcd_renderer::{GpuCpuMetrics, LcdBrushes, LcdRenderer};
use crate::rendering::primitives::{Color, Point, Rect};
use crate::rendering::renderer::Renderer;
use crate::rendering::TypographyRole;

pub const WINDOW_CLASS_NAME: PCWSTR = w!("AIPulse_MainWindow");

pub struct MainWindow {
    pub hwnd: HWND,
    pub d2d: Direct2DContext,
    pub tray: Option<TrayIcon>,
    pub app: Arc<Application>,
    pub dpi: u32,
    pub lcd_renderer: Arc<LcdRenderer>,
    pub lcd_brushes: Option<LcdBrushes>,
}

impl MainWindow {
    pub fn create_and_run(app: Arc<Application>) -> Result<()> {
        unsafe {
            DpiContext::enable_dpi_awareness();

            let hinstance = GetModuleHandleW(None)?;
            let hcursor = LoadCursorW(None, IDC_ARROW)?;
            let hicon = LoadIconW(None, IDI_APPLICATION)?;

            let wnd_class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(Self::wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance.into(),
                hIcon: hicon,
                hCursor: hcursor,
                hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(core::ptr::null_mut()),
                lpszMenuName: PCWSTR::null(),
                lpszClassName: WINDOW_CLASS_NAME,
                hIconSm: hicon,
            };

            RegisterClassExW(&wnd_class);

            let sys_dpi = GetDpiForSystem();
            let dpi_scale = (sys_dpi as f32 / 96.0).max(1.0);

            // Base client size in DIPs: 960 x 680
            let client_w = (960.0 * dpi_scale) as i32;
            let client_h = (680.0 * dpi_scale) as i32;

            let mut win_rect = RECT {
                left: 100,
                top: 100,
                right: 100 + client_w,
                bottom: 100 + client_h,
            };

            let win_style = WS_OVERLAPPEDWINDOW;
            let _ = AdjustWindowRectExForDpi(&mut win_rect, win_style, false, WINDOW_EX_STYLE::default(), sys_dpi);

            let initial_width = win_rect.right - win_rect.left;
            let initial_height = win_rect.bottom - win_rect.top;

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                WINDOW_CLASS_NAME,
                w!("AIPulse - NZXT Kraken 360 Controller"),
                win_style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                initial_width,
                initial_height,
                None,
                None,
                Some(hinstance.into()),
                None,
            )?;

            // Dark mode title bar for Windows 10/11
            let dark_mode = BOOL(1);
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE(20), // DWMWA_USE_IMMERSIVE_DARK_MODE
                &dark_mode as *const _ as *const _,
                std::mem::size_of::<BOOL>() as u32,
            );

            let dwrite = DirectWriteContext::new().map_err(|e| {
                error!("DirectWrite init failed: {}", e);
                e
            })?;

            let lcd_renderer = LcdRenderer::new(&dwrite.factory).map_err(|e| {
                error!("LcdRenderer init failed: {}", e);
                e
            })?;

            let d2d = Direct2DContext::new(hwnd, dwrite).map_err(|e| {
                error!("Direct2D init failed: {}", e);
                e
            })?;

            let dpi = DpiContext::get_for_window(hwnd);
            let tray = TrayIcon::new(hwnd, "AIPulse - Kraken 360");

            let window_state = Box::new(MainWindow {
                hwnd,
                d2d,
                tray: Some(tray),
                app: Arc::clone(&app),
                dpi,
                lcd_renderer,
                lcd_brushes: None,
            });

            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(window_state) as isize);

            // Start background threads
            app.start_workers(hwnd);

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);

            info!("Main Win32 window created and visible (DPI: {}).", dpi);
            Ok(())
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;

            if !ptr.is_null() {
                let window = &mut *ptr;
                match msg {
                    WM_PAINT => {
                        let mut ps = PAINTSTRUCT::default();
                        let _hdc = BeginPaint(hwnd, &mut ps);

                        window.render();

                        let _ = EndPaint(hwnd, &ps);
                        return LRESULT(0);
                    }
                    WM_SIZE => {
                        let width = (lparam.0 & 0xffff) as u32;
                        let height = ((lparam.0 >> 16) & 0xffff) as u32;
                        let _ = window.d2d.resize(width, height);
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        return LRESULT(0);
                    }
                    WM_GETMINMAXINFO => {
                        let minmax = &mut *(lparam.0 as *mut MINMAXINFO);
                        let dpi_scale = window.dpi as f32 / 96.0;
                        minmax.ptMinTrackSize.x = (820.0 * dpi_scale) as i32;
                        minmax.ptMinTrackSize.y = (580.0 * dpi_scale) as i32;
                        return LRESULT(0);
                    }
                    WM_DPICHANGED => {
                        let new_dpi = (wparam.0 & 0xffff) as u32;
                        window.dpi = new_dpi;
                        window.d2d.set_dpi(new_dpi as f32, new_dpi as f32);

                        let rect = *(lparam.0 as *const RECT);
                        let _ = SetWindowPos(
                            hwnd,
                            None,
                            rect.left,
                            rect.top,
                            rect.right - rect.left,
                            rect.bottom - rect.top,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        return LRESULT(0);
                    }
                    WM_APP_UPDATE => {
                        let _ = InvalidateRect(Some(hwnd), None, false);
                        return LRESULT(0);
                    }
                    WM_TRAYICON => {
                        let event = (lparam.0 & 0xffff) as u32;
                        if event == WM_LBUTTONUP || event == WM_LBUTTONDBLCLK {
                            let _ = ShowWindow(hwnd, SW_RESTORE);
                            let _ = SetForegroundWindow(hwnd);
                        }
                        return LRESULT(0);
                    }
                    WM_CLOSE => {
                        let minimize_to_tray = {
                            if let Ok(state) = window.app.state.read() {
                                state.config.minimize_to_tray
                            } else {
                                true
                            }
                        };

                        if minimize_to_tray {
                            let _ = ShowWindow(hwnd, SW_HIDE);
                            return LRESULT(0);
                        } else {
                            let _ = DestroyWindow(hwnd);
                            return LRESULT(0);
                        }
                    }
                    WM_DESTROY => {
                        info!("Window destroying. Cleaning up resources...");
                        window.app.shutdown();
                        if let Some(ref mut tray) = window.tray {
                            tray.remove();
                        }
                        drop(Box::from_raw(ptr));
                        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                        PostQuitMessage(0);
                        return LRESULT(0);
                    }
                    _ => {}
                }
            }

            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }

    fn render(&mut self) {
        // Fast immutable snapshot read
        let snapshot = {
            if let Ok(guard) = self.app.state.read() {
                guard.clone()
            } else {
                return;
            }
        };

        if self.d2d.begin_frame().is_err() {
            return;
        }

        self.d2d.clear(Color::BG_DARK);

        let mut client_rect = RECT::default();
        unsafe {
            let _ = GetClientRect(self.hwnd, &mut client_rect);
        }

        // Convert physical client pixel dimensions to logical DIPs
        let dpi_scale = (self.dpi as f32 / 96.0).max(0.5);
        let total_w = (client_rect.right - client_rect.left) as f32 / dpi_scale;
        let total_h = (client_rect.bottom - client_rect.top) as f32 / dpi_scale;

        // In DIP units (Direct2D automatically scales these to physical pixels)
        let pad = 20.0;

        // 1. Header Bar
        let _ = self.d2d.draw_text(
            "AIPulse",
            &Rect::from_points(pad, pad, 200.0, 28.0),
            Color::TEXT_PRIMARY,
            TypographyRole::AppTitle,
        );
        let _ = self.d2d.draw_text(
            "NZXT Kraken 360 Hardware Controller • Native Windows 11",
            &Rect::from_points(pad, pad + 26.0, 450.0, 18.0),
            Color::TEXT_MUTED,
            TypographyRole::Caption,
        );

        // Connection Status Pill (top right)
        let pill_w = 200.0;
        let pill_h = 30.0;
        let pill_rect = Rect::from_points(total_w - pad - pill_w, pad + 2.0, pill_w, pill_h);
        let (status_text, dot_color) = match snapshot.kraken.status {
            DeviceConnectionStatus::Connected => ("● Kraken 360 Connected", Color::ACCENT_GREEN),
            DeviceConnectionStatus::Simulated => ("● Kraken 360 (Simulated)", Color::ACCENT_CYAN),
            DeviceConnectionStatus::Disconnected => ("● Kraken Disconnected", Color::ACCENT_RED),
        };

        let _ = self.d2d.fill_rounded_rect(&pill_rect, 15.0, Color::CARD_SURFACE);
        let _ = self.d2d.draw_rounded_rect(&pill_rect, 15.0, Color::CARD_BORDER, 1.0);
        let _ = self.d2d.draw_text(
            status_text,
            &pill_rect.inset(10.0, 6.0),
            dot_color,
            TypographyRole::CardHeader,
        );

        // Divider
        let divider_y = pad + 52.0;
        let _ = self.d2d.draw_line(
            Point::new(pad, divider_y),
            Point::new(total_w - pad, divider_y),
            Color::CARD_BORDER,
            1.0,
        );

        // Content Area: Two Column Layout
        let content_y = divider_y + 14.0;
        let footer_h = 32.0;
        let content_h = (total_h - content_y - footer_h - 8.0).max(300.0);

        let col_gap = 16.0;
        let left_w = ((total_w - pad * 2.0 - col_gap) * 0.54).max(280.0);
        let right_w = total_w - pad * 2.0 - col_gap - left_w;

        // LEFT COLUMN: 3 Sensor Cards
        let card_gap = 10.0;
        let card_h = (content_h - card_gap * 2.0) / 3.0;

        // Card 1: CPU
        let cpu_card = Rect::from_points(pad, content_y, left_w, card_h);
        self.render_telemetry_card(
            &cpu_card,
            "CPU Telemetry",
            &format!("{:.1}°", snapshot.sensors.cpu_temperature),
            &format!("{:.0}% Usage • {:.0} MHz", snapshot.sensors.cpu_usage, snapshot.sensors.cpu_clock_mhz),
            snapshot.sensors.cpu_usage / 100.0,
            Color::ACCENT_GREEN,
        );

        // Card 2: GPU
        let gpu_card = Rect::from_points(pad, content_y + card_h + card_gap, left_w, card_h);
        let gpu_title = if snapshot.sensors.gpu_name.is_empty() {
            "GPU Telemetry".to_string()
        } else {
            format!("GPU Telemetry • {}", snapshot.sensors.gpu_name)
        };
        self.render_telemetry_card(
            &gpu_card,
            &gpu_title,
            &format!("{:.1}°", snapshot.sensors.gpu_temperature),
            &format!("{:.0}% Usage • {:.0} MB VRAM", snapshot.sensors.gpu_usage, snapshot.sensors.gpu_vram_mb),
            snapshot.sensors.gpu_usage / 100.0,
            Color::ACCENT_PURPLE,
        );

        // Card 3: Cooling & Kraken Telemetry
        let aio_card = Rect::from_points(pad, content_y + (card_h + card_gap) * 2.0, left_w, card_h);
        self.render_telemetry_card(
            &aio_card,
            "Liquid Coolant & Pump Telemetry",
            &format!("{:.1}°", snapshot.kraken.liquid_temperature),
            &format!("Pump: {} RPM • Fans: {} RPM", snapshot.kraken.pump_rpm, snapshot.kraken.fan_rpm),
            ((snapshot.kraken.liquid_temperature - 20.0) / 30.0).clamp(0.0, 1.0),
            Color::ACCENT_CYAN,
        );

        // RIGHT COLUMN: Live Kraken 360 LCD Preview
        let right_x = pad + left_w + col_gap;
        let lcd_card = Rect::from_points(right_x, content_y, right_w, content_h);
        self.render_lcd_preview(&lcd_card, &snapshot);

        // FOOTER BAR
        let footer_y = total_h - footer_h;
        let _ = self.d2d.draw_line(
            Point::new(pad, footer_y),
            Point::new(total_w - pad, footer_y),
            Color::CARD_BORDER,
            1.0,
        );

        let footer_text = format!(
            "NZXT VID: 0x1E71 • Firmware: {} • Serial: {} • Direct2D Accelerated",
            snapshot.kraken.firmware_version, snapshot.kraken.serial_number
        );
        let _ = self.d2d.draw_text(
            &footer_text,
            &Rect::from_points(pad, footer_y + 8.0, total_w - pad * 2.0, 18.0),
            Color::TEXT_MUTED,
            TypographyRole::Caption,
        );

        let _ = self.d2d.end_frame();
    }

    fn render_telemetry_card(
        &mut self,
        card: &Rect,
        title: &str,
        big_value: &str,
        subtitle: &str,
        bar_fraction: f32,
        bar_color: Color,
    ) {
        let _ = self.d2d.fill_rounded_rect(card, 8.0, Color::CARD_SURFACE);
        let _ = self.d2d.draw_rounded_rect(card, 8.0, Color::CARD_BORDER, 1.0);

        let pad_x = 16.0;
        let pad_y = 12.0;

        // Card Title
        let _ = self.d2d.draw_text(
            title,
            &Rect::from_points(card.left + pad_x, card.top + pad_y, card.width() - pad_x * 2.0, 18.0),
            Color::TEXT_SECONDARY,
            TypographyRole::CardHeader,
        );

        // Big Primary Value
        let _ = self.d2d.draw_text(
            big_value,
            &Rect::from_points(card.left + pad_x, card.top + pad_y + 20.0, 150.0, 30.0),
            Color::TEXT_PRIMARY,
            TypographyRole::MetricLarge,
        );

        // Subtitle Info
        let _ = self.d2d.draw_text(
            subtitle,
            &Rect::from_points(card.left + pad_x, card.top + pad_y + 52.0, card.width() - pad_x * 2.0, 18.0),
            Color::TEXT_MUTED,
            TypographyRole::Body,
        );

        // Progress bar at bottom of card
        let bar_h = 4.0;
        let bar_y = card.bottom - pad_y - bar_h;
        let bar_w = card.width() - pad_x * 2.0;

        let bg_bar = Rect::from_points(card.left + pad_x, bar_y, bar_w, bar_h);
        let _ = self.d2d.fill_rounded_rect(&bg_bar, 2.0, Color::CARD_SURFACE_LIGHT);

        let fill_w = bar_w * bar_fraction.clamp(0.0, 1.0);
        if fill_w > 0.0 {
            let fill_bar = Rect::from_points(card.left + pad_x, bar_y, fill_w, bar_h);
            let _ = self.d2d.fill_rounded_rect(&fill_bar, 2.0, bar_color);
        }
    }

    fn render_lcd_preview(
        &mut self,
        card: &Rect,
        snapshot: &crate::app::state::AppState,
    ) {
        let _ = self.d2d.fill_rounded_rect(card, 8.0, Color::CARD_SURFACE);
        let _ = self.d2d.draw_rounded_rect(card, 8.0, Color::CARD_BORDER, 1.0);

        let pad_x = 16.0;
        let pad_y = 12.0;

        // Card Title
        let _ = self.d2d.draw_text(
            "Kraken 360 LCD Display",
            &Rect::from_points(card.left + pad_x, card.top + pad_y, card.width() - pad_x * 2.0, 18.0),
            Color::TEXT_SECONDARY,
            TypographyRole::CardHeader,
        );

        let subtitle = format!(
            "Active: {} • 5s Rotation Cycle",
            snapshot.lcd.active_infographic.name()
        );
        let _ = self.d2d.draw_text(
            &subtitle,
            &Rect::from_points(card.left + pad_x, card.top + pad_y + 18.0, card.width() - pad_x * 2.0, 16.0),
            Color::TEXT_MUTED,
            TypographyRole::Caption,
        );

        // Cooler cap circular bezel center
        let center_x = card.left + card.width() * 0.5;
        let center_y = card.top + card.height() * 0.53;
        let max_r_w = card.width() * 0.40;
        let max_r_h = (card.height() - 60.0) * 0.42;
        let outer_cap_radius = max_r_w.min(max_r_h).clamp(40.0, 120.0);

        // 1. Dark matte aluminum outer bezel ring
        let _ = self.d2d.fill_ellipse(
            Point::new(center_x, center_y),
            outer_cap_radius + 12.0,
            outer_cap_radius + 12.0,
            Color::from_hex(0x13151a),
        );
        let _ = self.d2d.draw_ellipse(
            Point::new(center_x, center_y),
            outer_cap_radius + 12.0,
            outer_cap_radius + 12.0,
            Color::from_hex(0x2d3240),
            2.0,
        );

        // 2. Bezel groove / step ring
        let _ = self.d2d.draw_ellipse(
            Point::new(center_x, center_y),
            outer_cap_radius + 4.0,
            outer_cap_radius + 4.0,
            Color::from_hex(0x0a0c0f),
            2.5,
        );

        // 3. LCD screen viewport (circular display)
        let lcd_rect = Rect::from_points(
            center_x - outer_cap_radius,
            center_y - outer_cap_radius,
            outer_cap_radius * 2.0,
            outer_cap_radius * 2.0,
        );

        // 4. Render LCD display:
        // Render directly via Direct2D in lockstep with the telemetry cards
        // using the exact synchronized snapshot state.
        let metrics = GpuCpuMetrics::new(snapshot.sensors.cpu_usage, snapshot.sensors.gpu_usage);
        if let Some(ref rt) = self.d2d.render_target.clone() {
            if self.lcd_brushes.is_none() {
                self.lcd_brushes = LcdBrushes::new(rt).ok();
            }
            if let Some(ref brushes) = self.lcd_brushes {
                match snapshot.lcd.active_infographic {
                    crate::nzxt::lcd::InfographicType::CpuGpuLoad => {
                        let _ = self.lcd_renderer.render(rt, &lcd_rect, brushes, &metrics);
                    }
                    crate::nzxt::lcd::InfographicType::CpuGpuTemperature => {
                        let _ = self.lcd_renderer.render_temperature_infographic(
                            rt,
                            &lcd_rect,
                            brushes,
                            snapshot.sensors.cpu_temperature,
                            snapshot.sensors.gpu_temperature,
                            snapshot.kraken.liquid_temperature,
                        );
                    }
                }
            }
        } else if !snapshot.lcd.preview_buffer.is_empty()
            && snapshot.lcd.preview_buffer.iter().any(|&b| b != 0)
        {
            let _ = self.d2d.draw_bitmap_buffer(
                &lcd_rect,
                &snapshot.lcd.preview_buffer,
                snapshot.lcd.width,
                snapshot.lcd.height,
            );
        }

        // Glass reflection / inner bezel highlight rim
        let _ = self.d2d.draw_ellipse(
            Point::new(center_x, center_y),
            outer_cap_radius,
            outer_cap_radius,
            Color::rgba(1.0, 1.0, 1.0, 0.15),
            1.5,
        );
    }
}
