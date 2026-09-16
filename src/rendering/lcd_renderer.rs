use std::sync::Arc;
use windows::core::{w, Result};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows_numerics::Matrix3x2;

use super::primitives::{Color, Rect};

pub const CANONICAL_SIZE: f32 = 330.0;
pub const VIEWPORT_FILL_SCALE: f32 = 1.85;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuCpuMetrics {
    pub cpu_usage: f32,
    pub gpu_usage: f32,
}

impl GpuCpuMetrics {
    pub fn new(cpu_usage: f32, gpu_usage: f32) -> Self {
        Self {
            cpu_usage: cpu_usage.clamp(0.0, 100.0),
            gpu_usage: gpu_usage.clamp(0.0, 100.0),
        }
    }
}

impl Default for GpuCpuMetrics {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            gpu_usage: 0.0,
        }
    }
}

pub struct LcdBrushes {
    pub bg_brush: ID2D1SolidColorBrush,
    pub white_brush: ID2D1SolidColorBrush,
    pub bar_track_brush: ID2D1SolidColorBrush,
    pub cpu_fill_brush: ID2D1SolidColorBrush,
    pub gpu_fill_brush: ID2D1SolidColorBrush,
    pub coolant_brush: ID2D1SolidColorBrush,
    pub cpu_temp_green_brush: ID2D1SolidColorBrush,
    pub cpu_temp_amber_brush: ID2D1SolidColorBrush,
    pub cpu_temp_red_brush: ID2D1SolidColorBrush,
    pub gpu_temp_brush: ID2D1SolidColorBrush,
    pub brand_brush: ID2D1SolidColorBrush,
    pub temp_bg_brush: ID2D1SolidColorBrush,
    pub gauge_track_brush: ID2D1SolidColorBrush,
}

impl LcdBrushes {
    pub fn new(rt: &ID2D1RenderTarget) -> Result<Self> {
        unsafe {
            // Colors matching exact visual reference
            // Pure Black background
            let bg_color = Color::rgb(0.0, 0.0, 0.0).to_d2d();
            let bg_brush = rt.CreateSolidColorBrush(&bg_color, None)?;

            // Pure White text (#FFFFFF)
            let white_color = Color::rgb(1.0, 1.0, 1.0).to_d2d();
            let white_brush = rt.CreateSolidColorBrush(&white_color, None)?;

            // Bar background empty track (#898989)
            let bar_track_color = Color::from_hex(0x898989).to_d2d();
            let bar_track_brush = rt.CreateSolidColorBrush(&bar_track_color, None)?;

            // CPU bar fill purple (#6A00DF)
            let cpu_fill_color = Color::from_hex(0x6a00df).to_d2d();
            let cpu_fill_brush = rt.CreateSolidColorBrush(&cpu_fill_color, None)?;

            // GPU bar fill magenta (#D700C0)
            let gpu_fill_color = Color::from_hex(0xd700c0).to_d2d();
            let gpu_fill_brush = rt.CreateSolidColorBrush(&gpu_fill_color, None)?;

            // Coolant cyan (#00D2FF)
            let coolant_color = Color::from_hex(0x00d2ff).to_d2d();
            let coolant_brush = rt.CreateSolidColorBrush(&coolant_color, None)?;

            // Temperature threshold colors
            let green_color = Color::from_hex(0x10b981).to_d2d();
            let cpu_temp_green_brush = rt.CreateSolidColorBrush(&green_color, None)?;

            let amber_color = Color::from_hex(0xf59e0b).to_d2d();
            let cpu_temp_amber_brush = rt.CreateSolidColorBrush(&amber_color, None)?;

            let red_color = Color::from_hex(0xef4444).to_d2d();
            let cpu_temp_red_brush = rt.CreateSolidColorBrush(&red_color, None)?;

            // GPU temp purple (#8B5CF6)
            let gpu_temp_color = Color::from_hex(0x8b5cf6).to_d2d();
            let gpu_temp_brush = rt.CreateSolidColorBrush(&gpu_temp_color, None)?;

            // Brand text gray (#575E70)
            let brand_color = Color::from_hex(0x575e70).to_d2d();
            let brand_brush = rt.CreateSolidColorBrush(&brand_color, None)?;

            // Temperature background dark base (#0E1014)
            let temp_bg_color = Color::from_hex(0x0e1014).to_d2d();
            let temp_bg_brush = rt.CreateSolidColorBrush(&temp_bg_color, None)?;

            // Circular gauge track background (#20242E)
            let gauge_track_color = Color::from_hex(0x20242e).to_d2d();
            let gauge_track_brush = rt.CreateSolidColorBrush(&gauge_track_color, None)?;

            Ok(Self {
                bg_brush,
                white_brush,
                bar_track_brush,
                cpu_fill_brush,
                gpu_fill_brush,
                coolant_brush,
                cpu_temp_green_brush,
                cpu_temp_amber_brush,
                cpu_temp_red_brush,
                gpu_temp_brush,
                brand_brush,
                temp_bg_brush,
                gauge_track_brush,
            })
        }
    }
}

pub struct LcdRenderer {
    pub label_top_format: IDWriteTextFormat,
    pub label_bot_format: IDWriteTextFormat,
    pub top_metric_format: IDWriteTextFormat,
    pub bot_metric_format: IDWriteTextFormat,
    pub top_pct_format: IDWriteTextFormat,
    pub bot_pct_format: IDWriteTextFormat,
    pub temp_primary_format: IDWriteTextFormat,
    pub temp_subtitle_format: IDWriteTextFormat,
    pub temp_brand_format: IDWriteTextFormat,
}

impl LcdRenderer {
    pub fn new(dwrite_factory: &IDWriteFactory) -> Result<Arc<Self>> {
        unsafe {
            // 1. Label format for "CPU" (Left/Bottom aligned above bar)
            let label_top_format = dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                15.0,
                w!("en-US"),
            )?;
            label_top_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            label_top_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_FAR)?;

            // 2. Label format for "GPU" (Left/Top aligned below bar)
            let label_bot_format = dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                15.0,
                w!("en-US"),
            )?;
            label_bot_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
            label_bot_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;

            // 3. Top metric format (Large bold, Right/Bottom aligned to hug upper bar)
            let top_metric_format = dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                62.0,
                w!("en-US"),
            )?;
            top_metric_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING)?;
            top_metric_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_FAR)?;

            // 4. Bottom metric format (Large bold, Right/Top aligned to hug lower bar)
            let bot_metric_format = dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                62.0,
                w!("en-US"),
            )?;
            bot_metric_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING)?;
            bot_metric_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;

            // 5. Top "%" format (Small bold, Right/Bottom aligned to match baseline)
            let top_pct_format = dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                18.0,
                w!("en-US"),
            )?;
            top_pct_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING)?;
            top_pct_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_FAR)?;

            // 6. Bottom "%" format (Small bold, Right/Bottom aligned to match baseline of large number)
            let bot_pct_format = dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                18.0,
                w!("en-US"),
            )?;
            bot_pct_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING)?;
            bot_pct_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_FAR)?;

            // 7. Temperature Primary Metric format (Large bold centered, e.g. "42°C")
            let temp_primary_format = dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                46.0,
                w!("en-US"),
            )?;
            temp_primary_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            temp_primary_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            // 8. Temperature Subtitle format (Centered, e.g. "GPU 56°C")
            let temp_subtitle_format = dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                16.0,
                w!("en-US"),
            )?;
            temp_subtitle_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            temp_subtitle_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            // 9. Temperature Brand format (Centered, e.g. "AIPulse")
            let temp_brand_format = dwrite_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                13.0,
                w!("en-US"),
            )?;
            temp_brand_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            temp_brand_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;

            Ok(Arc::new(Self {
                label_top_format,
                label_bot_format,
                top_metric_format,
                bot_metric_format,
                top_pct_format,
                bot_pct_format,
                temp_primary_format,
                temp_subtitle_format,
                temp_brand_format,
            }))
        }
    }

    /// Renders static elements in canonical coordinates (330x330):
    /// - Black circular background
    /// - CPU label
    /// - GPU label
    /// - Top and bottom "%" symbols
    /// - Gray rounded-rectangle bar tracks
    pub fn render_static(&self, rt: &ID2D1RenderTarget, brushes: &LcdBrushes) -> Result<()> {
        self.render_static_with_unit(rt, brushes, "%")
    }

    /// Renders static elements with parameterized unit symbol ("%" or "°C")
    pub fn render_static_with_unit(
        &self,
        rt: &ID2D1RenderTarget,
        brushes: &LcdBrushes,
        unit: &str,
    ) -> Result<()> {
        unsafe {
            let cx = CANONICAL_SIZE * 0.5;
            let cy = CANONICAL_SIZE * 0.5;
            let radius = (CANONICAL_SIZE * 0.5) / VIEWPORT_FILL_SCALE;

            // 1. Black circular LCD background
            let ellipse = D2D1_ELLIPSE {
                point: windows_numerics::Vector2 { X: cx, Y: cy },
                radiusX: radius,
                radiusY: radius,
            };
            rt.FillEllipse(&ellipse, &brushes.bg_brush);

            // 2. Bar tracks (gray rounded rects)
            // Dimensions: width 134, height 13, corner radius 6.5
            let cpu_bar_rounded = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: 98.0,
                    top: 148.5,
                    right: 232.0,
                    bottom: 161.5,
                },
                radiusX: 6.5,
                radiusY: 6.5,
            };
            rt.FillRoundedRectangle(&cpu_bar_rounded, &brushes.bar_track_brush);

            let gpu_bar_rounded = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: 98.0,
                    top: 168.5,
                    right: 232.0,
                    bottom: 181.5,
                },
                radiusX: 6.5,
                radiusY: 6.5,
            };
            rt.FillRoundedRectangle(&gpu_bar_rounded, &brushes.bar_track_brush);

            // 3. Static "CPU" label (snug above upper bar)
            let cpu_label_utf16: Vec<u16> = "CPU".encode_utf16().collect();
            let cpu_label_rect = D2D_RECT_F {
                left: 98.0,
                top: 125.0,
                right: 145.0,
                bottom: 146.0,
            };
            rt.DrawText(
                &cpu_label_utf16,
                &self.label_top_format,
                &cpu_label_rect,
                &brushes.white_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );

            // 4. Static "GPU" label (snug below lower bar)
            let gpu_label_utf16: Vec<u16> = "GPU".encode_utf16().collect();
            let gpu_label_rect = D2D_RECT_F {
                left: 98.0,
                top: 184.0,
                right: 145.0,
                bottom: 202.0,
            };
            rt.DrawText(
                &gpu_label_utf16,
                &self.label_bot_format,
                &gpu_label_rect,
                &brushes.white_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );

            let unit_left = if unit == "%" { 217.0 } else { 205.0 };

            // 5. Static top unit symbol (e.g. "%" or "°C") aligned to baseline of top number
            let unit_utf16: Vec<u16> = unit.encode_utf16().collect();
            let top_unit_rect = D2D_RECT_F {
                left: unit_left,
                top: 110.0,
                right: 232.0,
                bottom: 150.5,
            };
            rt.DrawText(
                &unit_utf16,
                &self.top_pct_format,
                &top_unit_rect,
                &brushes.white_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );

            // 6. Static bottom unit symbol (e.g. "%" or "°C") aligned to baseline of bottom number
            let bot_unit_rect = D2D_RECT_F {
                left: unit_left,
                top: 175.0,
                right: 232.0,
                bottom: 228.0,
            };
            rt.DrawText(
                &unit_utf16,
                &self.bot_pct_format,
                &bot_unit_rect,
                &brushes.white_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );

            Ok(())
        }
    }

    /// Renders dynamic elements in canonical coordinates (330x330):
    /// - CPU percentage number
    /// - GPU percentage number
    /// - Clipped CPU progress bar fill (purple)
    /// - Clipped GPU progress bar fill (magenta)
    pub fn render_dynamic(
        &self,
        rt: &ID2D1RenderTarget,
        brushes: &LcdBrushes,
        metrics: &GpuCpuMetrics,
    ) -> Result<()> {
        self.render_dynamic_values(rt, brushes, metrics.cpu_usage, metrics.gpu_usage, "%")
    }

    /// Renders dynamic values and progress bars with parameterized unit ("%" or "°C")
    pub fn render_dynamic_values(
        &self,
        rt: &ID2D1RenderTarget,
        brushes: &LcdBrushes,
        top_val: f32,
        bot_val: f32,
        unit: &str,
    ) -> Result<()> {
        unsafe {
            // 1. Dynamic CPU bar fill (clipped to rounded rectangle)
            let cpu_bar_rounded = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: 98.0,
                    top: 148.5,
                    right: 232.0,
                    bottom: 161.5,
                },
                radiusX: 6.5,
                radiusY: 6.5,
            };

            let cpu_pct = (top_val / 100.0).clamp(0.0, 1.0);
            let cpu_fill_width = 134.0 * cpu_pct;
            if cpu_fill_width > 0.0 {
                let clip_rect = D2D_RECT_F {
                    left: 98.0,
                    top: 148.5,
                    right: 98.0 + cpu_fill_width,
                    bottom: 161.5,
                };
                rt.PushAxisAlignedClip(&clip_rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                rt.FillRoundedRectangle(&cpu_bar_rounded, &brushes.cpu_fill_brush);
                rt.PopAxisAlignedClip();
            }

            // 2. Dynamic GPU bar fill (clipped to rounded rectangle)
            let gpu_bar_rounded = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F {
                    left: 98.0,
                    top: 168.5,
                    right: 232.0,
                    bottom: 181.5,
                },
                radiusX: 6.5,
                radiusY: 6.5,
            };

            let gpu_pct = (bot_val / 100.0).clamp(0.0, 1.0);
            let gpu_fill_width = 134.0 * gpu_pct;
            if gpu_fill_width > 0.0 {
                let clip_rect = D2D_RECT_F {
                    left: 98.0,
                    top: 168.5,
                    right: 98.0 + gpu_fill_width,
                    bottom: 181.5,
                };
                rt.PushAxisAlignedClip(&clip_rect, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
                rt.FillRoundedRectangle(&gpu_bar_rounded, &brushes.gpu_fill_brush);
                rt.PopAxisAlignedClip();
            }

            let metric_right = if unit == "%" { 217.0 } else { 205.0 };

            // 3. Dynamic top usage/temperature value (snug above upper bar)
            let cpu_str = format!("{:.0}", top_val.max(0.0).round());
            let cpu_utf16: Vec<u16> = cpu_str.encode_utf16().collect();
            let cpu_val_rect = D2D_RECT_F {
                left: 80.0,
                top: 80.0,
                right: metric_right,
                bottom: 161.0,
            };
            rt.DrawText(
                &cpu_utf16,
                &self.top_metric_format,
                &cpu_val_rect,
                &brushes.white_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );

            // 4. Dynamic bottom usage/temperature value (snug below lower bar)
            let gpu_str = format!("{:.0}", bot_val.max(0.0).round());
            let gpu_utf16: Vec<u16> = gpu_str.encode_utf16().collect();
            let gpu_val_rect = D2D_RECT_F {
                left: 80.0,
                top: 163.0,
                right: metric_right,
                bottom: 237.0,
            };
            rt.DrawText(
                &gpu_utf16,
                &self.bot_metric_format,
                &gpu_val_rect,
                &brushes.white_brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );

            Ok(())
        }
    }

    /// Renders complete LCD into target destination rect, maintaining exact 1:1 aspect ratio.
    pub fn render(
        &self,
        rt: &ID2D1RenderTarget,
        dest_rect: &Rect,
        brushes: &LcdBrushes,
        metrics: &GpuCpuMetrics,
    ) -> Result<()> {
        unsafe {
            // Save current transform
            let mut old_transform = Matrix3x2::default();
            rt.GetTransform(&mut old_transform);

            // Preserve 1:1 circular aspect ratio, scaled to fill the viewport
            let base_scale = (dest_rect.width() / CANONICAL_SIZE).min(dest_rect.height() / CANONICAL_SIZE);
            let scale = base_scale * VIEWPORT_FILL_SCALE;

            let dest_cx = dest_rect.left + dest_rect.width() * 0.5;
            let dest_cy = dest_rect.top + dest_rect.height() * 0.5;
            let canon_cx = CANONICAL_SIZE * 0.5;
            let canon_cy = CANONICAL_SIZE * 0.5;

            let offset_x = dest_cx - canon_cx * scale;
            let offset_y = dest_cy - canon_cy * scale;

            let new_transform = Matrix3x2 {
                M11: scale,
                M12: 0.0,
                M21: 0.0,
                M22: scale,
                M31: offset_x,
                M32: offset_y,
            };

            rt.SetTransform(&new_transform);

            // Static elements pass
            self.render_static(rt, brushes)?;

            // Dynamic elements pass
            self.render_dynamic(rt, brushes, metrics)?;

            // Restore transform
            rt.SetTransform(&old_transform);

            Ok(())
        }
    }

    #[allow(dead_code)]
    fn draw_arc_ring(
        &self,
        rt: &ID2D1RenderTarget,
        cx: f32,
        cy: f32,
        radius: f32,
        thickness: f32,
        pct: f32,
        brush: &ID2D1SolidColorBrush,
        track_brush: &ID2D1SolidColorBrush,
    ) -> Result<()> {
        unsafe {
            // 1. Draw full track background ring
            let ellipse = D2D1_ELLIPSE {
                point: windows_numerics::Vector2 { X: cx, Y: cy },
                radiusX: radius,
                radiusY: radius,
            };
            rt.DrawEllipse(&ellipse, track_brush, thickness, None);

            // 2. Draw active arc percentage
            let clamped = pct.clamp(0.0, 1.0);
            if clamped <= 0.005 {
                return Ok(());
            }
            if clamped >= 0.995 {
                rt.DrawEllipse(&ellipse, brush, thickness, None);
                return Ok(());
            }

            let start_angle = -std::f32::consts::FRAC_PI_2;
            let sweep = clamped * std::f32::consts::TAU;
            let end_angle = start_angle + sweep;

            let start_pt = windows_numerics::Vector2 {
                X: cx + radius * start_angle.cos(),
                Y: cy + radius * start_angle.sin(),
            };
            let end_pt = windows_numerics::Vector2 {
                X: cx + radius * end_angle.cos(),
                Y: cy + radius * end_angle.sin(),
            };

            let factory = rt.GetFactory()?;
            let geom = factory.CreatePathGeometry()?;
            let sink = geom.Open()?;

            sink.BeginFigure(start_pt, D2D1_FIGURE_BEGIN_HOLLOW);
            sink.AddArc(&D2D1_ARC_SEGMENT {
                point: end_pt,
                size: D2D_SIZE_F {
                    width: radius,
                    height: radius,
                },
                rotationAngle: 0.0,
                sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
                arcSize: if clamped > 0.5 {
                    D2D1_ARC_SIZE_LARGE
                } else {
                    D2D1_ARC_SIZE_SMALL
                },
            });
            sink.EndFigure(D2D1_FIGURE_END_OPEN);
            sink.Close()?;

            rt.DrawGeometry(&geom, brush, thickness, None);
            Ok(())
        }
    }

    /// Renders temperature infographic elements in canonical coordinates (330x330),
    /// using the identical dual-bar design as % load with CPU top, GPU bottom, and "°C" units.
    pub fn render_temperature_canonical(
        &self,
        rt: &ID2D1RenderTarget,
        brushes: &LcdBrushes,
        cpu_temp: f32,
        gpu_temp: f32,
        _liquid_temp: f32,
    ) -> Result<()> {
        self.render_static_with_unit(rt, brushes, "°C")?;
        self.render_dynamic_values(rt, brushes, cpu_temp, gpu_temp, "°C")?;
        Ok(())
    }

    /// Renders complete Temperature LCD into target destination rect, maintaining 1:1 aspect ratio.
    pub fn render_temperature_infographic(
        &self,
        rt: &ID2D1RenderTarget,
        dest_rect: &Rect,
        brushes: &LcdBrushes,
        cpu_temp: f32,
        gpu_temp: f32,
        liquid_temp: f32,
    ) -> Result<()> {
        unsafe {
            let mut old_transform = Matrix3x2::default();
            rt.GetTransform(&mut old_transform);

            let base_scale = (dest_rect.width() / CANONICAL_SIZE).min(dest_rect.height() / CANONICAL_SIZE);
            let scale = base_scale * VIEWPORT_FILL_SCALE;

            let dest_cx = dest_rect.left + dest_rect.width() * 0.5;
            let dest_cy = dest_rect.top + dest_rect.height() * 0.5;
            let canon_cx = CANONICAL_SIZE * 0.5;
            let canon_cy = CANONICAL_SIZE * 0.5;

            let offset_x = dest_cx - canon_cx * scale;
            let offset_y = dest_cy - canon_cy * scale;

            let new_transform = Matrix3x2 {
                M11: scale,
                M12: 0.0,
                M21: 0.0,
                M22: scale,
                M31: offset_x,
                M32: offset_y,
            };

            rt.SetTransform(&new_transform);

            self.render_temperature_canonical(rt, brushes, cpu_temp, gpu_temp, liquid_temp)?;

            rt.SetTransform(&old_transform);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_cpu_metrics_clamping() {
        let m1 = GpuCpuMetrics::new(-10.0, 150.0);
        assert_eq!(m1.cpu_usage, 0.0);
        assert_eq!(m1.gpu_usage, 100.0);

        let m2 = GpuCpuMetrics::new(42.0, 67.0);
        assert_eq!(m2.cpu_usage, 42.0);
        assert_eq!(m2.gpu_usage, 67.0);
    }

    #[test]
    fn test_bar_fill_geometry_proportions() {
        let total_bar_w: f32 = 134.0;
        let test_cases: [(f32, f32); 5] = [
            (0.0, 0.0),
            (25.0, 33.5),
            (50.0, 67.0),
            (75.0, 100.5),
            (100.0, 134.0),
        ];

        for (pct, expected_w) in test_cases {
            let fill_w = total_bar_w * (pct / 100.0).clamp(0.0, 1.0);
            assert!((fill_w - expected_w).abs() < 1e-4, "Mismatch for {}%", pct);
        }
    }

    #[test]
    fn test_canonical_aspect_ratio_scaling() {
        let dest_square = Rect::from_points(0.0, 0.0, 240.0, 240.0);
        let base_scale = (dest_square.width() / CANONICAL_SIZE).min(dest_square.height() / CANONICAL_SIZE);
        let scale = base_scale * VIEWPORT_FILL_SCALE;
        assert!((scale - (240.0 / 330.0 * VIEWPORT_FILL_SCALE)).abs() < 1e-5);

        let dest_rect = Rect::from_points(10.0, 20.0, 400.0, 300.0);
        let base_scale2 = (dest_rect.width() / CANONICAL_SIZE).min(dest_rect.height() / CANONICAL_SIZE);
        let scale2 = base_scale2 * VIEWPORT_FILL_SCALE;
        assert_eq!(scale2, (300.0 / 330.0) * VIEWPORT_FILL_SCALE);
    }
}

