use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Devices::Usb::{
    WinUsb_Free, WinUsb_Initialize, WinUsb_WritePipe, WINUSB_INTERFACE_HANDLE,
};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InfographicType {
    CpuGpuLoad,
    CpuGpuTemperature,
}

impl InfographicType {
    pub const ALL: [InfographicType; 2] = [
        InfographicType::CpuGpuLoad,
        InfographicType::CpuGpuTemperature,
    ];

    pub fn next(&self) -> Self {
        match self {
            Self::CpuGpuLoad => Self::CpuGpuTemperature,
            Self::CpuGpuTemperature => Self::CpuGpuLoad,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::CpuGpuLoad => "CPU/GPU Load",
            Self::CpuGpuTemperature => "CPU/GPU Temperature",
        }
    }
}

pub struct InfographicRotationManager {
    current: InfographicType,
    last_rotation: Instant,
    rotation_duration: Duration,
}

impl InfographicRotationManager {
    pub const DEFAULT_ROTATION_SECS: u64 = 5;
    pub const CLEAR_TRANSITION_MS: u64 = 100;

    pub fn new(initial: InfographicType) -> Self {
        Self::with_duration(initial, Duration::from_secs(Self::DEFAULT_ROTATION_SECS))
    }

    pub fn with_duration(initial: InfographicType, duration: Duration) -> Self {
        Self {
            current: initial,
            last_rotation: Instant::now(),
            rotation_duration: duration,
        }
    }

    pub fn current(&self) -> InfographicType {
        self.current
    }

    pub fn set_current(&mut self, infographic: InfographicType, now: Instant) {
        if self.current != infographic {
            self.current = infographic;
            self.last_rotation = now;
        }
    }

    pub fn reset_timer(&mut self, now: Instant) {
        self.last_rotation = now;
    }

    pub fn rotation_duration(&self) -> Duration {
        self.rotation_duration
    }

    pub fn set_rotation_duration(&mut self, duration: Duration) {
        self.rotation_duration = duration;
    }

    /// Checks whether rotation interval has elapsed.
    /// If rotation interval has elapsed:
    /// - Automatically switches to the next infographic in the cycle.
    /// - Resets the 5-second rotation timer to `now`.
    /// - Returns true.
    /// Otherwise returns false.
    pub fn tick(&mut self, now: Instant) -> bool {
        if now.duration_since(self.last_rotation) >= self.rotation_duration {
            self.current = self.current.next();
            self.last_rotation = now;
            true
        } else {
            false
        }
    }

    pub fn elapsed(&self, now: Instant) -> Duration {
        now.duration_since(self.last_rotation)
    }

    pub fn time_remaining(&self, now: Instant) -> Duration {
        self.rotation_duration.saturating_sub(now.duration_since(self.last_rotation))
    }
}

#[derive(Debug, Error)]
pub enum LcdError {
    #[error("Invalid frame buffer dimensions: expected {expected}, got {actual}")]
    BufferMismatch { expected: usize, actual: usize },
    #[error("Hardware transport error: {0}")]
    Hardware(String),
}

pub trait Display: Send {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn submit_frame(&mut self, frame: &[u8]) -> Result<(), LcdError>;

    fn clear_screen(&mut self) -> Result<(), LcdError> {
        let mut blank = vec![0u8; (self.width() * self.height() * 4) as usize];
        for chunk in blank.chunks_exact_mut(4) {
            chunk[3] = 255;
        }
        self.submit_frame(&blank)
    }
}

pub struct SimulatedDisplay {
    width: u32,
    height: u32,
    framebuffer: Arc<RwLock<Vec<u8>>>,
}

impl SimulatedDisplay {
    pub fn new(width: u32, height: u32) -> (Self, Arc<RwLock<Vec<u8>>>) {
        let size = (width * height * 4) as usize;
        let buffer = Arc::new(RwLock::new(vec![0u8; size]));
        let display = Self {
            width,
            height,
            framebuffer: Arc::clone(&buffer),
        };
        (display, buffer)
    }
}

impl Display for SimulatedDisplay {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn submit_frame(&mut self, frame: &[u8]) -> Result<(), LcdError> {
        let expected = (self.width * self.height * 4) as usize;
        if frame.len() != expected {
            return Err(LcdError::BufferMismatch {
                expected,
                actual: frame.len(),
            });
        }
        if let Ok(mut lock) = self.framebuffer.write() {
            lock.copy_from_slice(frame);
        }
        Ok(())
    }
}

// 5x7 ASCII font table (ASCII 32 to 126)
const FONT_5X7: [[u8; 5]; 96] = [
    [0x00, 0x00, 0x00, 0x00, 0x00], // space
    [0x00, 0x00, 0x5F, 0x00, 0x00], // !
    [0x00, 0x07, 0x00, 0x07, 0x00], // "
    [0x14, 0x7F, 0x14, 0x7F, 0x14], // #
    [0x24, 0x2A, 0x7F, 0x2A, 0x12], // $
    [0x23, 0x13, 0x08, 0x64, 0x62], // %
    [0x36, 0x49, 0x55, 0x22, 0x50], // &
    [0x00, 0x05, 0x03, 0x00, 0x00], // '
    [0x00, 0x1C, 0x22, 0x41, 0x00], // (
    [0x00, 0x41, 0x22, 0x1C, 0x00], // )
    [0x14, 0x08, 0x3E, 0x08, 0x14], // *
    [0x08, 0x08, 0x3E, 0x08, 0x08], // +
    [0x00, 0x50, 0x30, 0x00, 0x00], // ,
    [0x08, 0x08, 0x08, 0x08, 0x08], // -
    [0x00, 0x60, 0x60, 0x00, 0x00], // .
    [0x20, 0x10, 0x08, 0x04, 0x02], // /
    [0x3E, 0x51, 0x49, 0x45, 0x3E], // 0
    [0x00, 0x42, 0x7F, 0x40, 0x00], // 1
    [0x42, 0x61, 0x51, 0x49, 0x46], // 2
    [0x21, 0x41, 0x45, 0x4B, 0x31], // 3
    [0x18, 0x14, 0x12, 0x7F, 0x10], // 4
    [0x27, 0x45, 0x45, 0x45, 0x39], // 5
    [0x3C, 0x4A, 0x49, 0x49, 0x30], // 6
    [0x01, 0x71, 0x09, 0x05, 0x03], // 7
    [0x36, 0x49, 0x49, 0x49, 0x36], // 8
    [0x06, 0x49, 0x49, 0x29, 0x1E], // 9
    [0x00, 0x36, 0x36, 0x00, 0x00], // :
    [0x00, 0x56, 0x36, 0x00, 0x00], // ;
    [0x08, 0x14, 0x22, 0x41, 0x00], // <
    [0x14, 0x14, 0x14, 0x14, 0x14], // =
    [0x00, 0x41, 0x22, 0x14, 0x08], // >
    [0x02, 0x01, 0x51, 0x09, 0x06], // ?
    [0x32, 0x49, 0x79, 0x41, 0x3E], // @
    [0x7E, 0x11, 0x11, 0x11, 0x7E], // A
    [0x7F, 0x49, 0x49, 0x49, 0x36], // B
    [0x3E, 0x41, 0x41, 0x41, 0x22], // C
    [0x7F, 0x41, 0x41, 0x22, 0x1C], // D
    [0x7F, 0x49, 0x49, 0x49, 0x41], // E
    [0x7F, 0x09, 0x09, 0x09, 0x01], // F
    [0x3E, 0x41, 0x49, 0x49, 0x7A], // G
    [0x7F, 0x08, 0x08, 0x08, 0x7F], // H
    [0x00, 0x41, 0x7F, 0x41, 0x00], // I
    [0x20, 0x40, 0x41, 0x3F, 0x01], // J
    [0x7F, 0x08, 0x14, 0x22, 0x41], // K
    [0x7F, 0x40, 0x40, 0x40, 0x40], // L
    [0x7F, 0x02, 0x0C, 0x02, 0x7F], // M
    [0x7F, 0x04, 0x08, 0x10, 0x7F], // N
    [0x3E, 0x41, 0x41, 0x41, 0x3E], // O
    [0x7F, 0x09, 0x09, 0x09, 0x06], // P
    [0x3E, 0x41, 0x51, 0x21, 0x5E], // Q
    [0x7F, 0x09, 0x19, 0x29, 0x46], // R
    [0x46, 0x49, 0x49, 0x49, 0x31], // S
    [0x01, 0x01, 0x7F, 0x01, 0x01], // T
    [0x3F, 0x40, 0x40, 0x40, 0x3F], // U
    [0x1F, 0x20, 0x40, 0x20, 0x1F], // V
    [0x7F, 0x20, 0x18, 0x20, 0x7F], // W
    [0x63, 0x14, 0x08, 0x14, 0x63], // X
    [0x07, 0x08, 0x70, 0x08, 0x07], // Y
    [0x61, 0x51, 0x49, 0x45, 0x43], // Z
    [0x00, 0x7F, 0x41, 0x41, 0x00], // [
    [0x02, 0x04, 0x08, 0x10, 0x20], // \
    [0x00, 0x41, 0x41, 0x7F, 0x00], // ]
    [0x04, 0x02, 0x01, 0x02, 0x04], // ^
    [0x40, 0x40, 0x40, 0x40, 0x40], // _
    [0x00, 0x01, 0x02, 0x04, 0x00], // `
    [0x20, 0x54, 0x54, 0x54, 0x78], // a
    [0x7F, 0x48, 0x44, 0x44, 0x38], // b
    [0x38, 0x44, 0x44, 0x44, 0x20], // c
    [0x38, 0x44, 0x44, 0x48, 0x7F], // d
    [0x38, 0x54, 0x54, 0x54, 0x18], // e
    [0x08, 0x7E, 0x09, 0x01, 0x02], // f
    [0x0C, 0x52, 0x52, 0x52, 0x3E], // g
    [0x7F, 0x08, 0x04, 0x04, 0x78], // h
    [0x00, 0x44, 0x7D, 0x40, 0x00], // i
    [0x20, 0x40, 0x44, 0x3D, 0x00], // j
    [0x7F, 0x10, 0x28, 0x44, 0x00], // k
    [0x00, 0x41, 0x7F, 0x40, 0x00], // l
    [0x7C, 0x04, 0x18, 0x04, 0x78], // m
    [0x7C, 0x08, 0x04, 0x04, 0x78], // n
    [0x38, 0x44, 0x44, 0x44, 0x38], // o
    [0x7C, 0x14, 0x14, 0x14, 0x08], // p
    [0x08, 0x14, 0x14, 0x18, 0x7C], // q
    [0x7C, 0x08, 0x04, 0x04, 0x08], // r
    [0x48, 0x54, 0x54, 0x54, 0x20], // s
    [0x04, 0x3F, 0x44, 0x40, 0x20], // t
    [0x3C, 0x40, 0x40, 0x20, 0x7C], // u
    [0x1C, 0x20, 0x40, 0x20, 0x1C], // v
    [0x3C, 0x40, 0x30, 0x40, 0x3C], // w
    [0x44, 0x28, 0x10, 0x28, 0x44], // x
    [0x0C, 0x50, 0x50, 0x50, 0x3C], // y
    [0x44, 0x64, 0x54, 0x4C, 0x44], // z
    [0x00, 0x08, 0x36, 0x41, 0x00], // {
    [0x00, 0x00, 0x7F, 0x00, 0x00], // |
    [0x00, 0x41, 0x36, 0x08, 0x00], // }
    [0x08, 0x08, 0x2A, 0x1C, 0x08], // ~ (degree symbol approximation)
    [0x00, 0x06, 0x09, 0x09, 0x06], // degree symbol at index 95
];

pub struct LcdFramebuffer {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl LcdFramebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height * 4) as usize;
        Self {
            width,
            height,
            pixels: vec![0u8; size],
        }
    }

    pub fn clear(&mut self, r: u8, g: u8, b: u8) {
        for chunk in self.pixels.chunks_exact_mut(4) {
            chunk[0] = r;
            chunk[1] = g;
            chunk[2] = b;
            chunk[3] = 255;
        }
    }

    pub fn clear_screen(&mut self) {
        self.clear(0, 0, 0);
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
        if x < self.width && y < self.height {
            let idx = ((y * self.width + x) * 4) as usize;
            self.pixels[idx] = r;
            self.pixels[idx + 1] = g;
            self.pixels[idx + 2] = b;
            self.pixels[idx + 3] = a;
        }
    }

    pub fn draw_ring(&mut self, cx: f32, cy: f32, radius: f32, thickness: f32, percent: f32, r: u8, g: u8, b: u8) {
        let r_inner = radius - thickness * 0.5;
        let r_outer = radius + thickness * 0.5;
        let r_inner_sq = r_inner * r_inner;
        let r_outer_sq = r_outer * r_outer;

        let max_angle = percent.clamp(0.0, 1.0) * std::f32::consts::TAU;

        let x_min = (cx - r_outer).max(0.0) as u32;
        let x_max = (cx + r_outer).min(self.width as f32 - 1.0) as u32;
        let y_min = (cy - r_outer).max(0.0) as u32;
        let y_max = (cy + r_outer).min(self.height as f32 - 1.0) as u32;

        for y in y_min..=y_max {
            for x in x_min..=x_max {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq >= r_inner_sq && dist_sq <= r_outer_sq {
                    let mut angle = (-dx).atan2(dy) + std::f32::consts::PI;
                    if angle < 0.0 {
                        angle += std::f32::consts::TAU;
                    }
                    if angle <= max_angle {
                        self.set_pixel(x, y, r, g, b, 255);
                    } else {
                        // Track background
                        self.set_pixel(x, y, 32, 36, 46, 255);
                    }
                }
            }
        }
    }

    pub fn draw_char(&mut self, ch: char, start_x: u32, start_y: u32, scale: u32, r: u8, g: u8, b: u8) {
        let glyph_idx = match ch {
            ' '..='~' => (ch as usize) - 32,
            '°' | '\u{00BA}' => 95,
            _ => return,
        };

        if glyph_idx >= FONT_5X7.len() {
            return;
        }

        let glyph = FONT_5X7[glyph_idx];
        for (col_idx, &col_byte) in glyph.iter().enumerate() {
            for row_idx in 0..7 {
                if (col_byte & (1 << row_idx)) != 0 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let px = start_x + (col_idx as u32) * scale + dx;
                            let py = start_y + (row_idx as u32) * scale + dy;
                            self.set_pixel(px, py, r, g, b, 255);
                        }
                    }
                }
            }
        }
    }

    pub fn draw_string_centered(&mut self, s: &str, cy: u32, scale: u32, r: u8, g: u8, b: u8) {
        let char_w = 6 * scale;
        let total_w = (s.chars().count() as u32) * char_w;
        let start_x = if total_w < self.width {
            (self.width - total_w) / 2
        } else {
            0
        };

        for (i, ch) in s.chars().enumerate() {
            let cx = start_x + (i as u32) * char_w;
            self.draw_char(ch, cx, cy, scale, r, g, b);
        }
    }

    pub fn render_dual_bars(&mut self, top_val: f32, bot_val: f32, unit: &str) {
        self.clear(0, 0, 0); // Pure black LCD

        let bar_x = 71;
        let bar_w = 98;
        let cpu_y = 108;
        let gpu_y = 123;
        let bar_h = 9;

        // 1. Draw Gray bar tracks
        for y in 0..bar_h {
            for x in 0..bar_w {
                self.set_pixel(bar_x + x, cpu_y + y, 137, 137, 137, 255);
                self.set_pixel(bar_x + x, gpu_y + y, 137, 137, 137, 255);
            }
        }

        // 2. CPU Purple Fill
        let top_fill_w = ((top_val / 100.0).clamp(0.0, 1.0) * bar_w as f32) as u32;
        for y in 0..bar_h {
            for x in 0..top_fill_w {
                self.set_pixel(bar_x + x, cpu_y + y, 106, 0, 223, 255);
            }
        }

        // 3. GPU Magenta Fill
        let bot_fill_w = ((bot_val / 100.0).clamp(0.0, 1.0) * bar_w as f32) as u32;
        for y in 0..bar_h {
            for x in 0..bot_fill_w {
                self.set_pixel(bar_x + x, gpu_y + y, 215, 0, 192, 255);
            }
        }

        // 4. Labels
        self.draw_char('C', bar_x + 2, cpu_y - 12, 1, 255, 255, 255);
        self.draw_char('P', bar_x + 9, cpu_y - 12, 1, 255, 255, 255);
        self.draw_char('U', bar_x + 16, cpu_y - 12, 1, 255, 255, 255);

        self.draw_char('G', bar_x + 2, gpu_y + bar_h + 5, 1, 255, 255, 255);
        self.draw_char('P', bar_x + 9, gpu_y + bar_h + 5, 1, 255, 255, 255);
        self.draw_char('U', bar_x + 16, gpu_y + bar_h + 5, 1, 255, 255, 255);

        // 5. Unit symbols (e.g. "%" or "°C")
        if unit == "%" {
            self.draw_char('%', bar_x + bar_w - 7, cpu_y - 10, 1, 255, 255, 255);
            self.draw_char('%', bar_x + bar_w - 7, gpu_y + bar_h + 25, 1, 255, 255, 255);

            let cpu_str = format!("{:.0}", top_val.clamp(0.0, 100.0));
            let num_x = bar_x + bar_w - 12 - (cpu_str.len() as u32 * 24);
            for (i, ch) in cpu_str.chars().enumerate() {
                self.draw_char(ch, num_x + (i as u32 * 24), cpu_y - 32, 4, 255, 255, 255);
            }

            let gpu_str = format!("{:.0}", bot_val.clamp(0.0, 100.0));
            let num_gx = bar_x + bar_w - 12 - (gpu_str.len() as u32 * 24);
            for (i, ch) in gpu_str.chars().enumerate() {
                self.draw_char(ch, num_gx + (i as u32 * 24), gpu_y + bar_h + 4, 4, 255, 255, 255);
            }
        } else {
            // "°C"
            let unit_w = (unit.chars().count() as u32) * 6;
            let unit_x = bar_x + bar_w - unit_w;
            for (i, ch) in unit.chars().enumerate() {
                self.draw_char(ch, unit_x + (i as u32 * 6), cpu_y - 10, 1, 255, 255, 255);
                self.draw_char(ch, unit_x + (i as u32 * 6), gpu_y + bar_h + 25, 1, 255, 255, 255);
            }

            let cpu_str = format!("{:.0}", top_val.clamp(0.0, 100.0));
            let num_x = unit_x - 5 - (cpu_str.len() as u32 * 24);
            for (i, ch) in cpu_str.chars().enumerate() {
                self.draw_char(ch, num_x + (i as u32 * 24), cpu_y - 32, 4, 255, 255, 255);
            }

            let gpu_str = format!("{:.0}", bot_val.clamp(0.0, 100.0));
            let num_gx = unit_x - 5 - (gpu_str.len() as u32 * 24);
            for (i, ch) in gpu_str.chars().enumerate() {
                self.draw_char(ch, num_gx + (i as u32 * 24), gpu_y + bar_h + 4, 4, 255, 255, 255);
            }
        }
    }

    pub fn render_infographic(&mut self, cpu_temp: f32, gpu_temp: f32, _liquid_temp: f32) {
        self.render_dual_bars(cpu_temp, gpu_temp, "°C");
    }

    pub fn render_load(&mut self, cpu_usage: f32, gpu_usage: f32) {
        self.render_dual_bars(cpu_usage, gpu_usage, "%");
    }

    pub fn render_active_infographic(
        &mut self,
        infographic: InfographicType,
        cpu_usage: f32,
        gpu_usage: f32,
        cpu_temp: f32,
        gpu_temp: f32,
        liquid_temp: f32,
    ) {
        match infographic {
            InfographicType::CpuGpuLoad => self.render_load(cpu_usage, gpu_usage),
            InfographicType::CpuGpuTemperature => {
                self.render_infographic(cpu_temp, gpu_temp, liquid_temp);
            }
        }
    }
}

pub struct KrakenLcd {
    width: u32,
    height: u32,
    winusb: Option<WINUSB_INTERFACE_HANDLE>,
    file_handle: Option<HANDLE>,
    hid_device: Option<hidapi::HidDevice>,
    initialized: bool,
    last_connect_attempt: Option<Instant>,
}

impl KrakenLcd {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            winusb: None,
            file_handle: None,
            hid_device: None,
            initialized: false,
            last_connect_attempt: None,
        }
    }

    pub fn clear_screen(&mut self) -> Result<(), LcdError> {
        let mut blank = vec![0u8; (self.width * self.height * 4) as usize];
        for chunk in blank.chunks_exact_mut(4) {
            chunk[3] = 255;
        }
        self.submit_frame(&blank)
    }

    pub fn connect(&mut self) -> Result<(), LcdError> {
        info!("Connecting to Kraken 360 LCD hardware...");
        self.hid_device = None;

        // 1. Open HID device for control transfers
        if let Ok(api) = hidapi::HidApi::new() {
            for dev_info in api.device_list() {
                if dev_info.vendor_id() == 0x1E71
                    && (dev_info.product_id() == 0x300E || dev_info.product_id() == 0x300C)
                    && dev_info.interface_number() == 1
                {
                    if let Ok(dev) = api.open_path(dev_info.path()) {
                        info!("Kraken LCD: HID control interface opened.");
                        self.hid_device = Some(dev);
                        break;
                    }
                }
            }
        }

        // 2. Open WinUSB Bulk device for image transfers
        let winusb_path = r"\\?\USB#VID_1E71&PID_300E&MI_00#9&fb10aea&0&0000#{300e300d-7ee7-1125-0724-101503010819}";
        let hstring_path = HSTRING::from(winusb_path);

        unsafe {
            let handle = CreateFileW(
                PCWSTR(hstring_path.as_ptr()),
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED,
                None,
            );

            match handle {
                Ok(h) if !h.is_invalid() => {
                    let mut winusb = WINUSB_INTERFACE_HANDLE::default();
                    if WinUsb_Initialize(h, &mut winusb).is_ok() {
                        info!("Kraken LCD: WinUSB bulk interface initialized successfully.");
                        self.file_handle = Some(h);
                        self.winusb = Some(winusb);
                        return Ok(());
                    } else {
                        let _ = CloseHandle(h);
                        return Err(LcdError::Hardware("WinUsb_Initialize failed".into()));
                    }
                }
                Err(err) => {
                    return Err(LcdError::Hardware(format!("Could not open WinUSB device handle: {}", err)));
                }
                Ok(h) => {
                    let _ = CloseHandle(h);
                    return Err(LcdError::Hardware("Invalid device handle".into()));
                }
            }
        }
    }

    fn send_frame_internal(&mut self, rgb565_data: &[u8]) -> Result<(), LcdError> {
        let (winusb, hid) = match (self.winusb, &self.hid_device) {
            (Some(w), Some(h)) => (w, h),
            _ => return Err(LcdError::Hardware("Kraken LCD device not connected".into())),
        };

        unsafe {
            // 1. Send start data transfer via HID: [0x36, 0x01, 0x00, 0x01, 0x06]
            let mut start_cmd = [0u8; 64];
            start_cmd[0] = 0x36;
            start_cmd[1] = 0x01;
            start_cmd[2] = 0x00;
            start_cmd[3] = 0x01;
            start_cmd[4] = 0x06;
            let _ = hid.write(&start_cmd);

            let mut ack = [0u8; 64];
            let _ = hid.read_timeout(&mut ack, 100);

            // 2. Send Bulk OUT Header (20 bytes) to Pipe 0x02
            let data_len = rgb565_data.len() as u32;
            let mut header = Vec::with_capacity(20);
            header.extend_from_slice(&[
                0x12, 0xFA, 0x01, 0xE8, 0xAB, 0xCD, 0xEF, 0x98, 0x76, 0x54, 0x32, 0x10,
            ]);
            header.extend_from_slice(&[0x06, 0x00, 0x00, 0x00]);
            header.extend_from_slice(&data_len.to_le_bytes());

            let mut written = 0u32;
            let _ = WinUsb_WritePipe(winusb, 0x02, &header, Some(&mut written), None);

            // 3. Send Bulk OUT Payload (chunks of 4096 bytes)
            for chunk in rgb565_data.chunks(4096) {
                let _ = WinUsb_WritePipe(winusb, 0x02, chunk, Some(&mut written), None);
            }

            // 4. Send end data transfer via HID: [0x36, 0x02]
            let mut end_cmd = [0u8; 64];
            end_cmd[0] = 0x36;
            end_cmd[1] = 0x02;
            let _ = hid.write(&end_cmd);

            let _ = hid.read_timeout(&mut ack, 100);
        }

        Ok(())
    }
}

impl Display for KrakenLcd {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn submit_frame(&mut self, rgba_frame: &[u8]) -> Result<(), LcdError> {
        let expected = (self.width * self.height * 4) as usize;
        if rgba_frame.len() != expected {
            return Err(LcdError::BufferMismatch {
                expected,
                actual: rgba_frame.len(),
            });
        }

        if self.winusb.is_none() {
            let should_retry = self
                .last_connect_attempt
                .map_or(true, |t| t.elapsed() >= std::time::Duration::from_secs(5));
            if should_retry {
                self.last_connect_attempt = Some(std::time::Instant::now());
                let _ = self.connect();
            }
        }

        // Convert RGBA -> RGB565
        let mut rgb565 = Vec::with_capacity((self.width * self.height * 2) as usize);
        for chunk in rgba_frame.chunks_exact(4) {
            let r = (chunk[0] >> 3) as u16;
            let g = (chunk[1] >> 2) as u16;
            let b = (chunk[2] >> 3) as u16;

            let byte0 = ((r << 3) | (g >> 3)) as u8;
            let byte1 = (((g & 0x07) << 5) | b) as u8;

            rgb565.push(byte0);
            rgb565.push(byte1);
        }

        self.send_frame_internal(&rgb565)?;

        // If first initialization, transmit second time for hardware framebuffer swap
        if !self.initialized {
            let _ = self.send_frame_internal(&rgb565);
            self.initialized = true;
            info!("Kraken LCD hardware initial frame swap complete.");
        }

        Ok(())
    }
}

unsafe impl Send for KrakenLcd {}

impl Drop for KrakenLcd {
    fn drop(&mut self) {
        unsafe {
            if let Some(winusb) = self.winusb {
                let _ = WinUsb_Free(winusb);
            }
            if let Some(handle) = self.file_handle {
                let _ = CloseHandle(handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_framebuffer_lifecycle() {
        let mut fb = LcdFramebuffer::new(240, 240);
        assert_eq!(fb.pixels.len(), 240 * 240 * 4);

        fb.clear(10, 20, 30);
        assert_eq!(fb.pixels[0], 10);
        assert_eq!(fb.pixels[1], 20);
        assert_eq!(fb.pixels[2], 30);
        assert_eq!(fb.pixels[3], 255);

        fb.render_infographic(45.0, 55.0, 32.0);
        assert_eq!(fb.pixels.len(), 240 * 240 * 4);
    }

    #[test]
    fn test_simulated_display() {
        let (mut display, buffer) = SimulatedDisplay::new(64, 64);
        assert_eq!(display.width(), 64);
        assert_eq!(display.height(), 64);

        let test_frame = vec![128u8; 64 * 64 * 4];
        assert!(display.submit_frame(&test_frame).is_ok());

        let read_buf = buffer.read().unwrap();
        assert_eq!(read_buf[0], 128);

        let invalid_frame = vec![0u8; 100];
        assert!(display.submit_frame(&invalid_frame).is_err());
    }

    #[test]
    fn test_degree_symbol_glyph_and_render() {
        let mut fb = LcdFramebuffer::new(240, 240);

        // Verify glyph 95 exists and is non-empty
        assert_eq!(FONT_5X7.len(), 96);
        let deg_glyph = FONT_5X7[95];
        let bit_count: u32 = deg_glyph.iter().map(|b| b.count_ones()).sum();
        assert!(bit_count > 0, "Degree glyph at index 95 must contain active pixels");

        // Clear with pure black
        fb.clear(0, 0, 0);

        // Draw "42°C" centered at y=100 with 4x scale
        let text = "42°C";
        fb.draw_string_centered(text, 100, 4, 255, 255, 255);

        // Calculate expected character bounding boxes
        let scale = 4u32;
        let char_w = 6 * scale; // 24 px
        let total_w = (text.chars().count() as u32) * char_w; // 96 px
        let start_x = (240 - total_w) / 2; // 72 px

        let x_4 = start_x;                      // 72..96
        let x_2 = start_x + char_w;              // 96..120
        let x_deg = start_x + 2 * char_w;        // 120..144
        let x_c = start_x + 3 * char_w;          // 144..168

        // Verify each character has drawn white pixels in its expected column range
        let count_pixels_in_x_range = |x_start: u32, x_end: u32| -> usize {
            let mut count = 0;
            for y in 100..(100 + 7 * scale) {
                for x in x_start..x_end {
                    let idx = ((y * 240 + x) * 4) as usize;
                    if fb.pixels[idx] == 255 && fb.pixels[idx + 1] == 255 && fb.pixels[idx + 2] == 255 {
                        count += 1;
                    }
                }
            }
            count
        };

        let px_4 = count_pixels_in_x_range(x_4, x_2);
        let px_2 = count_pixels_in_x_range(x_2, x_deg);
        let px_deg = count_pixels_in_x_range(x_deg, x_c);
        let px_c = count_pixels_in_x_range(x_c, x_c + char_w);

        assert!(px_4 > 0, "Digit '4' must be rendered");
        assert!(px_2 > 0, "Digit '2' must be rendered");
        assert!(px_deg > 0, "Degree symbol '°' must be rendered");
        assert!(px_c > 0, "Letter 'C' must be rendered");

        // Verify relative position: '°' comes strictly before 'C'
        assert!(x_deg < x_c, "Degree symbol must precede 'C' horizontally");
    }

    #[test]
    fn test_infographic_rotation_cycle_order() {
        assert_eq!(InfographicType::ALL.len(), 2);
        assert_eq!(InfographicType::ALL[0], InfographicType::CpuGpuLoad);
        assert_eq!(InfographicType::ALL[1], InfographicType::CpuGpuTemperature);

        // Deterministic transitions
        let initial = InfographicType::CpuGpuLoad;
        let next1 = initial.next();
        assert_eq!(next1, InfographicType::CpuGpuTemperature);
        let next2 = next1.next();
        assert_eq!(next2, InfographicType::CpuGpuLoad);

        // Indefinite cycling test
        let mut cur = InfographicType::CpuGpuLoad;
        for _ in 0..10 {
            cur = cur.next();
            assert_eq!(cur, InfographicType::CpuGpuTemperature);
            cur = cur.next();
            assert_eq!(cur, InfographicType::CpuGpuLoad);
        }
    }

    #[test]
    fn test_infographic_rotation_manager_5_second_timer() {
        let t0 = Instant::now();
        let mut mgr = InfographicRotationManager::new(InfographicType::CpuGpuLoad);
        mgr.reset_timer(t0);

        assert_eq!(mgr.current(), InfographicType::CpuGpuLoad);
        assert_eq!(mgr.rotation_duration(), Duration::from_secs(5));

        // 1. At t = 2s: should NOT rotate
        let t_2s = t0 + Duration::from_secs(2);
        assert!(!mgr.tick(t_2s));
        assert_eq!(mgr.current(), InfographicType::CpuGpuLoad);
        assert_eq!(mgr.elapsed(t_2s), Duration::from_secs(2));

        // 2. At t = 4.9s: should NOT rotate
        let t_4_9s = t0 + Duration::from_millis(4900);
        assert!(!mgr.tick(t_4_9s));
        assert_eq!(mgr.current(), InfographicType::CpuGpuLoad);

        // 3. At t = 5.0s: exactly 5s elapsed -> MUST rotate to CpuGpuTemperature
        let t_5s = t0 + Duration::from_secs(5);
        assert!(mgr.tick(t_5s));
        assert_eq!(mgr.current(), InfographicType::CpuGpuTemperature);
        // Timer should have reset at t_5s
        assert_eq!(mgr.elapsed(t_5s), Duration::ZERO);

        // 4. At t = 7s (2s into temperature infographic): should NOT rotate
        let t_7s = t0 + Duration::from_secs(7);
        assert!(!mgr.tick(t_7s));
        assert_eq!(mgr.current(), InfographicType::CpuGpuTemperature);
        assert_eq!(mgr.elapsed(t_7s), Duration::from_secs(2));

        // 5. At t = 10s (5s after last rotation): MUST rotate back to CpuGpuLoad
        let t_10s = t0 + Duration::from_secs(10);
        assert!(mgr.tick(t_10s));
        assert_eq!(mgr.current(), InfographicType::CpuGpuLoad);
        assert_eq!(mgr.elapsed(t_10s), Duration::ZERO);

        // 6. Manual change: should reset the 5-second timer
        let t_12s = t0 + Duration::from_secs(12);
        mgr.set_current(InfographicType::CpuGpuTemperature, t_12s);
        assert_eq!(mgr.current(), InfographicType::CpuGpuTemperature);
        assert_eq!(mgr.elapsed(t_12s), Duration::ZERO);

        let t_16s = t0 + Duration::from_secs(16); // 4s after manual change -> should not rotate
        assert!(!mgr.tick(t_16s));
        assert_eq!(mgr.current(), InfographicType::CpuGpuTemperature);

        let t_17s = t0 + Duration::from_secs(17); // 5s after manual change -> must rotate to Load
        assert!(mgr.tick(t_17s));
        assert_eq!(mgr.current(), InfographicType::CpuGpuLoad);
    }

    #[test]
    fn test_independent_sensor_updates_within_rotation_cycle() {
        let mut fb = LcdFramebuffer::new(240, 240);
        let mut mgr = InfographicRotationManager::new(InfographicType::CpuGpuLoad);
        let t0 = Instant::now();
        mgr.reset_timer(t0);

        let sensor_refresh_interval = Duration::from_secs(1);
        let mut last_sensor_refresh = t0;

        let mut sensor_render_count = 0;
        let mut rotation_count = 0;

        // Simulate 20 seconds of operation at 100ms ticks
        for tick_idx in 1..=200 {
            let now = t0 + Duration::from_millis(tick_idx * 100);

            // 1. 5-second infographic rotation check
            if mgr.tick(now) {
                rotation_count += 1;
            }

            // 2. 1-second sensor refresh check (independent of 5s rotation)
            if now.duration_since(last_sensor_refresh) >= sensor_refresh_interval {
                sensor_render_count += 1;
                last_sensor_refresh = now;

                // Re-render active infographic with updated sensor telemetry
                let simulated_cpu = 20.0 + (tick_idx as f32 * 0.1);
                let simulated_gpu = 40.0 + (tick_idx as f32 * 0.1);
                fb.render_active_infographic(
                    mgr.current(),
                    simulated_cpu,
                    simulated_gpu,
                    simulated_cpu + 15.0,
                    simulated_gpu + 10.0,
                    32.0,
                );
                assert_eq!(fb.pixels.len(), 240 * 240 * 4);
            }
        }

        // In 20 seconds: exactly 4 rotations should occur (at 5s, 10s, 15s, 20s)
        assert_eq!(rotation_count, 4, "Should rotate exactly 4 times in 20 seconds (5s cycle)");
        // In 20 seconds: exactly 20 sensor refreshes should occur (1s interval)
        assert_eq!(sensor_render_count, 20, "Should refresh sensors exactly 20 times in 20 seconds (1s interval)");
        // Active infographic should be back to initial (Load) after 4 rotations (even number)
        assert_eq!(mgr.current(), InfographicType::CpuGpuLoad);
    }

    #[test]
    fn test_temperature_infographic_contains_degree_symbol_c() {
        let mut fb = LcdFramebuffer::new(240, 240);
        fb.render_infographic(42.0, 56.0, 32.0);

        // Verify the rendered frame contains non-black pixels from the infographic
        let non_zero_pixels = fb.pixels.chunks_exact(4).filter(|p| p[0] != 0 || p[1] != 0 || p[2] != 0).count();
        assert!(non_zero_pixels > 2000, "Temperature infographic must render graphics and text");

        // Verify dual bar colors (purple CPU bar and magenta GPU bar) matching % load infographic design
        let has_purple = fb.pixels.chunks_exact(4).any(|p| p[0] == 106 && p[1] == 0 && p[2] == 223);
        let has_magenta = fb.pixels.chunks_exact(4).any(|p| p[0] == 215 && p[1] == 0 && p[2] == 192);
        assert!(has_purple, "Should render purple CPU bar fill pixels");
        assert!(has_magenta, "Should render magenta GPU bar fill pixels");
    }

    #[test]
    fn test_clear_screen_before_infographic_jump() {
        let mut fb = LcdFramebuffer::new(240, 240);
        let mut mgr = InfographicRotationManager::new(InfographicType::CpuGpuLoad);
        let t0 = Instant::now();
        mgr.reset_timer(t0);

        // 1. Initial infographic (Load) rendered
        fb.render_load(45.0, 60.0);
        let non_zero_load = fb.pixels.chunks_exact(4).filter(|p| p[0] != 0 || p[1] != 0 || p[2] != 0).count();
        assert!(non_zero_load > 1000, "Initial load infographic has drawn pixels");

        // 2. At 5 seconds: rotation is triggered
        let t_5s = t0 + Duration::from_secs(5);
        assert!(mgr.tick(t_5s), "5s elapsed, should rotate");
        assert_eq!(mgr.current(), InfographicType::CpuGpuTemperature);

        // 3. Right before jumping to the next infographic: screen MUST be cleared to pure black
        fb.clear_screen();
        for chunk in fb.pixels.chunks_exact(4) {
            assert_eq!(chunk, [0, 0, 0, 255], "Screen must be cleared to pure black before jump");
        }

        // 4. Now render the new infographic (Temperature)
        fb.render_infographic(42.0, 56.0, 32.0);
        let non_zero_temp = fb.pixels.chunks_exact(4).filter(|p| p[0] != 0 || p[1] != 0 || p[2] != 0).count();
        assert!(non_zero_temp > 1000, "New temperature infographic has drawn pixels");
    }
}
