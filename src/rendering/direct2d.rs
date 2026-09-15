use std::collections::HashMap;
use std::sync::Arc;
use windows::core::{Result, HRESULT};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::directwrite::DirectWriteContext;
use super::primitives::{Color, Point, Rect};

pub struct Direct2DContext {
    pub factory: ID2D1Factory,
    pub dwrite: Arc<DirectWriteContext>,
    pub render_target: Option<ID2D1HwndRenderTarget>,
    pub hwnd: HWND,
    brushes: HashMap<u32, ID2D1SolidColorBrush>,
}

impl Direct2DContext {
    pub fn new(hwnd: HWND, dwrite: Arc<DirectWriteContext>) -> Result<Self> {
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let mut ctx = Self {
                factory,
                dwrite,
                render_target: None,
                hwnd,
                brushes: HashMap::new(),
            };
            ctx.ensure_render_target()?;
            Ok(ctx)
        }
    }

    pub fn ensure_render_target(&mut self) -> Result<()> {
        if self.render_target.is_some() {
            return Ok(());
        }

        unsafe {
            let mut rect = windows::Win32::Foundation::RECT::default();
            windows::Win32::UI::WindowsAndMessaging::GetClientRect(self.hwnd, &mut rect)?;

            let width = (rect.right - rect.left).max(1) as u32;
            let height = (rect.bottom - rect.top).max(1) as u32;

            let dpi = crate::windows::DpiContext::get_for_window(self.hwnd) as f32;

            let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: dpi,
                dpiY: dpi,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };

            let hwnd_rt_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd: self.hwnd,
                pixelSize: D2D_SIZE_U { width, height },
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };

            let rt = self.factory.CreateHwndRenderTarget(&rt_props, &hwnd_rt_props)?;
            self.render_target = Some(rt);
            self.brushes.clear();
            Ok(())
        }
    }

    pub fn set_dpi(&mut self, dpi_x: f32, dpi_y: f32) {
        if let Some(ref rt) = self.render_target {
            unsafe {
                rt.SetDpi(dpi_x, dpi_y);
            }
        }
    }

    pub fn discard_device_resources(&mut self) {
        self.brushes.clear();
        self.render_target = None;
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if let Some(ref rt) = self.render_target {
            unsafe {
                let size = D2D_SIZE_U {
                    width: width.max(1),
                    height: height.max(1),
                };
                let _ = rt.Resize(&size);
            }
        }
        Ok(())
    }

    pub fn get_brush(&mut self, color: Color) -> Result<ID2D1SolidColorBrush> {
        let key = ((color.r * 255.0) as u32) << 24
            | ((color.g * 255.0) as u32) << 16
            | ((color.b * 255.0) as u32) << 8
            | ((color.a * 255.0) as u32);

        if let Some(brush) = self.brushes.get(&key) {
            return Ok(brush.clone());
        }

        self.ensure_render_target()?;
        let rt = self.render_target.as_ref().unwrap();
        unsafe {
            let d2d_color = color.to_d2d();
            let brush = rt.CreateSolidColorBrush(&d2d_color as *const _, None)?;
            self.brushes.insert(key, brush.clone());
            Ok(brush)
        }
    }

    pub fn begin_draw(&mut self) -> Result<()> {
        self.ensure_render_target()?;
        if let Some(ref rt) = self.render_target {
            unsafe {
                rt.BeginDraw();
            }
        }
        Ok(())
    }

    pub fn end_draw(&mut self) -> Result<()> {
        if let Some(ref rt) = self.render_target {
            unsafe {
                let res = rt.EndDraw(None, None);
                if let Err(err) = res {
                    if err.code() == HRESULT(0x8899000C_u32 as i32) {
                        self.discard_device_resources();
                    } else {
                        return Err(err);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn clear(&self, color: Color) {
        if let Some(ref rt) = self.render_target {
            unsafe {
                let c = color.to_d2d();
                rt.Clear(Some(&c as *const _));
            }
        }
    }

    pub fn fill_rect(&mut self, rect: &Rect, color: Color) -> Result<()> {
        let brush = self.get_brush(color)?;
        if let Some(ref rt) = self.render_target {
            unsafe {
                let d2d_rect = D2D_RECT_F {
                    left: rect.left,
                    top: rect.top,
                    right: rect.right,
                    bottom: rect.bottom,
                };
                rt.FillRectangle(&d2d_rect, &brush);
            }
        }
        Ok(())
    }

    pub fn draw_rect(&mut self, rect: &Rect, color: Color, stroke_width: f32) -> Result<()> {
        let brush = self.get_brush(color)?;
        if let Some(ref rt) = self.render_target {
            unsafe {
                let d2d_rect = D2D_RECT_F {
                    left: rect.left,
                    top: rect.top,
                    right: rect.right,
                    bottom: rect.bottom,
                };
                rt.DrawRectangle(&d2d_rect, &brush, stroke_width, None);
            }
        }
        Ok(())
    }

    pub fn fill_rounded_rect(&mut self, rect: &Rect, radius: f32, color: Color) -> Result<()> {
        let brush = self.get_brush(color)?;
        if let Some(ref rt) = self.render_target {
            unsafe {
                let rounded = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: rect.left,
                        top: rect.top,
                        right: rect.right,
                        bottom: rect.bottom,
                    },
                    radiusX: radius,
                    radiusY: radius,
                };
                rt.FillRoundedRectangle(&rounded, &brush);
            }
        }
        Ok(())
    }

    pub fn draw_rounded_rect(
        &mut self,
        rect: &Rect,
        radius: f32,
        color: Color,
        stroke_width: f32,
    ) -> Result<()> {
        let brush = self.get_brush(color)?;
        if let Some(ref rt) = self.render_target {
            unsafe {
                let rounded = D2D1_ROUNDED_RECT {
                    rect: D2D_RECT_F {
                        left: rect.left,
                        top: rect.top,
                        right: rect.right,
                        bottom: rect.bottom,
                    },
                    radiusX: radius,
                    radiusY: radius,
                };
                rt.DrawRoundedRectangle(&rounded, &brush, stroke_width, None);
            }
        }
        Ok(())
    }

    pub fn draw_line(&mut self, p0: Point, p1: Point, color: Color, stroke_width: f32) -> Result<()> {
        let brush = self.get_brush(color)?;
        if let Some(ref rt) = self.render_target {
            unsafe {
                rt.DrawLine(p0.to_vector2(), p1.to_vector2(), &brush, stroke_width, None);
            }
        }
        Ok(())
    }

    pub fn fill_ellipse(&mut self, center: Point, rx: f32, ry: f32, color: Color) -> Result<()> {
        let brush = self.get_brush(color)?;
        if let Some(ref rt) = self.render_target {
            unsafe {
                let ellipse = D2D1_ELLIPSE {
                    point: center.to_vector2(),
                    radiusX: rx,
                    radiusY: ry,
                };
                rt.FillEllipse(&ellipse, &brush);
            }
        }
        Ok(())
    }

    pub fn draw_ellipse(
        &mut self,
        center: Point,
        rx: f32,
        ry: f32,
        color: Color,
        stroke_width: f32,
    ) -> Result<()> {
        let brush = self.get_brush(color)?;
        if let Some(ref rt) = self.render_target {
            unsafe {
                let ellipse = D2D1_ELLIPSE {
                    point: center.to_vector2(),
                    radiusX: rx,
                    radiusY: ry,
                };
                rt.DrawEllipse(&ellipse, &brush, stroke_width, None);
            }
        }
        Ok(())
    }

    pub fn draw_text(
        &mut self,
        text: &str,
        rect: &Rect,
        color: Color,
        role: super::directwrite::TypographyRole,
    ) -> Result<()> {
        let brush = self.get_brush(color)?;
        let format = self.dwrite.format_for_role(role);
        if let Some(ref rt) = self.render_target {
            unsafe {
                let utf16: Vec<u16> = text.encode_utf16().collect();
                let d2d_rect = D2D_RECT_F {
                    left: rect.left,
                    top: rect.top,
                    right: rect.right,
                    bottom: rect.bottom,
                };
                rt.DrawText(&utf16, format, &d2d_rect, &brush, D2D1_DRAW_TEXT_OPTIONS_NONE, DWRITE_MEASURING_MODE_NATURAL);
            }
        }
        Ok(())
    }

    pub fn draw_rgba_bitmap(
        &mut self,
        rect: &Rect,
        rgba_data: &[u8],
        width: u32,
        height: u32,
    ) -> Result<()> {
        self.ensure_render_target()?;
        if let Some(ref rt) = self.render_target {
            unsafe {
                let size = D2D_SIZE_U { width, height };
                let props = D2D1_BITMAP_PROPERTIES {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: 96.0,
                    dpiY: 96.0,
                };

                let pitch = width * 4;
                let bitmap = rt.CreateBitmap(size, Some(rgba_data.as_ptr() as *const _), pitch, &props)?;

                let dest_rect = D2D_RECT_F {
                    left: rect.left,
                    top: rect.top,
                    right: rect.right,
                    bottom: rect.bottom,
                };

                rt.DrawBitmap(
                    &bitmap,
                    Some(&dest_rect),
                    1.0,
                    D2D1_BITMAP_INTERPOLATION_MODE_LINEAR,
                    None,
                );
            }
        }
        Ok(())
    }
}
