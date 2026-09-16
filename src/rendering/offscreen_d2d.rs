use std::sync::Arc;
use windows::core::Result;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Gdi::*;

use super::lcd_renderer::{GpuCpuMetrics, LcdBrushes, LcdRenderer};
use super::primitives::{Color, Rect};

pub struct D2DOffscreenLcd {
    pub width: u32,
    pub height: u32,
    _factory: ID2D1Factory,
    pub renderer: Arc<LcdRenderer>,
    pub dc_rt: ID2D1DCRenderTarget,
    pub brushes: LcdBrushes,
    hdc: HDC,
    hbitmap: HBITMAP,
    prev_bitmap: HGDIOBJ,
    bits_ptr: *mut u8,
    pub rgba_buffer: Vec<u8>,
}

unsafe impl Send for D2DOffscreenLcd {}

impl D2DOffscreenLcd {
    pub fn new(width: u32, height: u32) -> Result<Self> {
        unsafe {
            // 1. Direct2D Factory and DirectWrite Factory
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let renderer = LcdRenderer::new(&dwrite_factory)?;

            // 2. Direct2D DC Render Target
            let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let dc_rt = factory.CreateDCRenderTarget(&rt_props)?;
            let brushes = LcdBrushes::new(&dc_rt)?;

            // 3. Create GDI Memory DC and 32-bit Top-down DIB Section
            let hdc = CreateCompatibleDC(None);
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32), // Top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default(); 1],
            };

            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let hbitmap = CreateDIBSection(
                Some(hdc),
                &bmi,
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            )?;

            let prev_bitmap = SelectObject(hdc, hbitmap.into());
            let size = (width * height * 4) as usize;
            let rgba_buffer = vec![0u8; size];

            Ok(Self {
                width,
                height,
                _factory: factory,
                renderer,
                dc_rt,
                brushes,
                hdc,
                hbitmap,
                prev_bitmap,
                bits_ptr: bits as *mut u8,
                rgba_buffer,
            })
        }
    }

    /// Renders frame using Direct2D pipeline and returns RGBA pixel buffer slice
    pub fn render_frame(&mut self, metrics: &GpuCpuMetrics) -> Result<&[u8]> {
        unsafe {
            let rect = RECT {
                left: 0,
                top: 0,
                right: self.width as i32,
                bottom: self.height as i32,
            };

            self.dc_rt.BindDC(self.hdc, &rect)?;
            self.dc_rt.BeginDraw();

            // Clear screen to solid black before rendering new frame
            let c = Color::rgb(0.0, 0.0, 0.0).to_d2d();
            self.dc_rt.Clear(Some(&c as *const _));

            let dest_rect = Rect::from_points(0.0, 0.0, self.width as f32, self.height as f32);
            self.renderer
                .render(&self.dc_rt, &dest_rect, &self.brushes, metrics)?;

            self.dc_rt.EndDraw(None, None)?;

            // Convert BGRA DIB pixels to RGBA buffer
            let total_pixels = (self.width * self.height) as usize;
            let bgra_slice = std::slice::from_raw_parts(self.bits_ptr, total_pixels * 4);

            for i in 0..total_pixels {
                let b = bgra_slice[i * 4];
                let g = bgra_slice[i * 4 + 1];
                let r = bgra_slice[i * 4 + 2];
                let a = bgra_slice[i * 4 + 3];

                self.rgba_buffer[i * 4] = r;
                self.rgba_buffer[i * 4 + 1] = g;
                self.rgba_buffer[i * 4 + 2] = b;
                self.rgba_buffer[i * 4 + 3] = a;
            }

            Ok(&self.rgba_buffer)
        }
    }

    /// Clears the offscreen LCD to solid black (RGBA: [0, 0, 0, 255])
    pub fn clear_frame(&mut self) -> Result<&[u8]> {
        unsafe {
            let rect = RECT {
                left: 0,
                top: 0,
                right: self.width as i32,
                bottom: self.height as i32,
            };

            self.dc_rt.BindDC(self.hdc, &rect)?;
            self.dc_rt.BeginDraw();
            let c = Color::rgb(0.0, 0.0, 0.0).to_d2d();
            self.dc_rt.Clear(Some(&c as *const _));
            self.dc_rt.EndDraw(None, None)?;

            for chunk in self.rgba_buffer.chunks_exact_mut(4) {
                chunk[0] = 0;
                chunk[1] = 0;
                chunk[2] = 0;
                chunk[3] = 255;
            }

            Ok(&self.rgba_buffer)
        }
    }

    /// Renders temperature infographic frame using Direct2D pipeline and returns RGBA pixel buffer slice
    pub fn render_temperature_frame(
        &mut self,
        cpu_temp: f32,
        gpu_temp: f32,
        liquid_temp: f32,
    ) -> Result<&[u8]> {
        unsafe {
            let rect = RECT {
                left: 0,
                top: 0,
                right: self.width as i32,
                bottom: self.height as i32,
            };

            self.dc_rt.BindDC(self.hdc, &rect)?;
            self.dc_rt.BeginDraw();

            // Clear screen to solid black before rendering new frame
            let c = Color::rgb(0.0, 0.0, 0.0).to_d2d();
            self.dc_rt.Clear(Some(&c as *const _));

            let dest_rect = Rect::from_points(0.0, 0.0, self.width as f32, self.height as f32);
            self.renderer.render_temperature_infographic(
                &self.dc_rt,
                &dest_rect,
                &self.brushes,
                cpu_temp,
                gpu_temp,
                liquid_temp,
            )?;

            self.dc_rt.EndDraw(None, None)?;

            // Convert BGRA DIB pixels to RGBA buffer
            let total_pixels = (self.width * self.height) as usize;
            let bgra_slice = std::slice::from_raw_parts(self.bits_ptr, total_pixels * 4);

            for i in 0..total_pixels {
                let b = bgra_slice[i * 4];
                let g = bgra_slice[i * 4 + 1];
                let r = bgra_slice[i * 4 + 2];
                let a = bgra_slice[i * 4 + 3];

                self.rgba_buffer[i * 4] = r;
                self.rgba_buffer[i * 4 + 1] = g;
                self.rgba_buffer[i * 4 + 2] = b;
                self.rgba_buffer[i * 4 + 3] = a;
            }

            Ok(&self.rgba_buffer)
        }
    }

    /// Unified rendering entrypoint by InfographicType
    pub fn render_infographic_frame(
        &mut self,
        infographic: crate::nzxt::lcd::InfographicType,
        metrics: &GpuCpuMetrics,
        cpu_temp: f32,
        gpu_temp: f32,
        liquid_temp: f32,
    ) -> Result<&[u8]> {
        match infographic {
            crate::nzxt::lcd::InfographicType::CpuGpuLoad => self.render_frame(metrics),
            crate::nzxt::lcd::InfographicType::CpuGpuTemperature => {
                self.render_temperature_frame(cpu_temp, gpu_temp, liquid_temp)
            }
        }
    }
}

impl Drop for D2DOffscreenLcd {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.hdc, self.prev_bitmap);
            let _ = DeleteObject(self.hbitmap.into());
            let _ = DeleteDC(self.hdc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offscreen_render_metrics() {
        let mut offscreen = D2DOffscreenLcd::new(240, 240).expect("Failed to create offscreen LCD");
        let metrics = GpuCpuMetrics::new(42.0, 67.0);
        let frame = offscreen.render_frame(&metrics).expect("Failed to render frame").to_vec();
        assert_eq!(frame.len(), 240 * 240 * 4);

        let metrics_zero = GpuCpuMetrics::new(0.0, 0.0);
        let _frame_zero = offscreen.render_frame(&metrics_zero).expect("Failed to render 0 frame");

        let mut has_purple = false;
        let mut has_magenta = false;
        for chunk in frame.chunks_exact(4) {
            // Purple: high B, low G, moderate R (#6A00DF)
            if chunk[2] > 180 && chunk[0] > 80 && chunk[1] < 40 {
                has_purple = true;
            }
            // Magenta: high R, high B, low G (#D700C0)
            if chunk[0] > 180 && chunk[2] > 160 && chunk[1] < 40 {
                has_magenta = true;
            }
        }
        assert!(has_purple, "Should render purple CPU bar fill pixels");
        assert!(has_magenta, "Should render magenta GPU bar fill pixels");
    }

    #[test]
    fn test_offscreen_render_temperature() {
        let mut offscreen = D2DOffscreenLcd::new(240, 240).expect("Failed to create offscreen LCD");
        let frame = offscreen
            .render_temperature_frame(42.0, 56.0, 32.0)
            .expect("Failed to render temperature frame")
            .to_vec();
        assert_eq!(frame.len(), 240 * 240 * 4);

        let non_zero_pixels = frame.chunks_exact(4).filter(|chunk| chunk[0] != 0 || chunk[1] != 0 || chunk[2] != 0).count();
        assert!(non_zero_pixels > 1000, "Temperature infographic should render graphics and text");

        // Verify dual bar design (purple CPU bar and magenta GPU bar) matches % load infographic
        let mut has_purple = false;
        let mut has_magenta = false;
        for chunk in frame.chunks_exact(4) {
            if chunk[2] > 180 && chunk[0] > 80 && chunk[1] < 40 {
                has_purple = true;
            }
            if chunk[0] > 180 && chunk[2] > 160 && chunk[1] < 40 {
                has_magenta = true;
            }
        }
        assert!(has_purple, "Temperature infographic should render purple CPU bar fill pixels");
        assert!(has_magenta, "Temperature infographic should render magenta GPU bar fill pixels");
    }

    #[test]
    fn test_offscreen_clear_frame() {
        let mut offscreen = D2DOffscreenLcd::new(240, 240).expect("Failed to create offscreen LCD");
        let frame = offscreen.clear_frame().expect("Failed to clear frame");
        assert_eq!(frame.len(), 240 * 240 * 4);
        for chunk in frame.chunks_exact(4) {
            assert_eq!(chunk, [0, 0, 0, 255]);
        }
    }

    #[test]
    fn test_offscreen_values_change_every_tick() {
        let mut offscreen = D2DOffscreenLcd::new(240, 240).expect("Failed to create offscreen LCD");
        let frame1 = offscreen
            .render_temperature_frame(42.0, 56.0, 32.0)
            .expect("Failed to render frame 1")
            .to_vec();

        let frame2 = offscreen
            .render_temperature_frame(48.0, 60.0, 33.0)
            .expect("Failed to render frame 2")
            .to_vec();

        assert_ne!(frame1, frame2, "Frames with different CPU/GPU values must produce distinct rendered frames");
    }
}

