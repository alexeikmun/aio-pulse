use super::direct2d::Direct2DContext;
use super::directwrite::TypographyRole;
use super::primitives::{Color, Point, Rect};
use windows::core::Result;

pub trait Renderer {
    fn begin_frame(&mut self) -> Result<()>;
    fn clear(&mut self, color: Color);
    fn fill_rect(&mut self, rect: &Rect, color: Color) -> Result<()>;
    fn draw_rect(&mut self, rect: &Rect, color: Color, stroke_width: f32) -> Result<()>;
    fn fill_rounded_rect(&mut self, rect: &Rect, radius: f32, color: Color) -> Result<()>;
    fn draw_rounded_rect(&mut self, rect: &Rect, radius: f32, color: Color, stroke_width: f32) -> Result<()>;
    fn draw_line(&mut self, p0: Point, p1: Point, color: Color, stroke_width: f32) -> Result<()>;
    fn fill_ellipse(&mut self, center: Point, rx: f32, ry: f32, color: Color) -> Result<()>;
    fn draw_ellipse(&mut self, center: Point, rx: f32, ry: f32, color: Color, stroke_width: f32) -> Result<()>;
    fn draw_text(&mut self, text: &str, rect: &Rect, color: Color, role: TypographyRole) -> Result<()>;
    fn draw_bitmap_buffer(&mut self, rect: &Rect, data: &[u8], width: u32, height: u32) -> Result<()>;
    fn end_frame(&mut self) -> Result<()>;
}

impl Renderer for Direct2DContext {
    fn begin_frame(&mut self) -> Result<()> {
        self.begin_draw()
    }

    fn clear(&mut self, color: Color) {
        Direct2DContext::clear(self, color);
    }

    fn fill_rect(&mut self, rect: &Rect, color: Color) -> Result<()> {
        Direct2DContext::fill_rect(self, rect, color)
    }

    fn draw_rect(&mut self, rect: &Rect, color: Color, stroke_width: f32) -> Result<()> {
        Direct2DContext::draw_rect(self, rect, color, stroke_width)
    }

    fn fill_rounded_rect(&mut self, rect: &Rect, radius: f32, color: Color) -> Result<()> {
        Direct2DContext::fill_rounded_rect(self, rect, radius, color)
    }

    fn draw_rounded_rect(&mut self, rect: &Rect, radius: f32, color: Color, stroke_width: f32) -> Result<()> {
        Direct2DContext::draw_rounded_rect(self, rect, radius, color, stroke_width)
    }

    fn draw_line(&mut self, p0: Point, p1: Point, color: Color, stroke_width: f32) -> Result<()> {
        Direct2DContext::draw_line(self, p0, p1, color, stroke_width)
    }

    fn fill_ellipse(&mut self, center: Point, rx: f32, ry: f32, color: Color) -> Result<()> {
        Direct2DContext::fill_ellipse(self, center, rx, ry, color)
    }

    fn draw_ellipse(&mut self, center: Point, rx: f32, ry: f32, color: Color, stroke_width: f32) -> Result<()> {
        Direct2DContext::draw_ellipse(self, center, rx, ry, color, stroke_width)
    }

    fn draw_text(&mut self, text: &str, rect: &Rect, color: Color, role: TypographyRole) -> Result<()> {
        Direct2DContext::draw_text(self, text, rect, color, role)
    }

    fn draw_bitmap_buffer(&mut self, rect: &Rect, data: &[u8], width: u32, height: u32) -> Result<()> {
        Direct2DContext::draw_rgba_bitmap(self, rect, data, width, height)
    }

    fn end_frame(&mut self) -> Result<()> {
        self.end_draw()
    }
}
