use windows_numerics::Vector2;
use windows::Win32::Graphics::Direct2D::Common::D2D1_COLOR_F;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub const fn to_vector2(&self) -> Vector2 {
        Vector2 { X: self.x, Y: self.y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Rect {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn from_points(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        }
    }

    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    pub fn center(&self) -> Point {
        Point::new(
            self.left + self.width() * 0.5,
            self.top + self.height() * 0.5,
        )
    }

    pub fn inset(&self, dx: f32, dy: f32) -> Self {
        Self {
            left: self.left + dx,
            top: self.top + dy,
            right: self.right - dx,
            bottom: self.bottom - dy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn from_hex(hex: u32) -> Self {
        let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
        let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
        let b = (hex & 0xFF) as f32 / 255.0;
        Self { r, g, b, a: 1.0 }
    }

    pub const fn to_d2d(&self) -> D2D1_COLOR_F {
        D2D1_COLOR_F {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a,
        }
    }

    // Palette
    pub const BG_DARK: Color = Color::from_hex(0x0f1115);
    pub const CARD_SURFACE: Color = Color::from_hex(0x171920);
    pub const CARD_SURFACE_LIGHT: Color = Color::from_hex(0x20232c);
    pub const CARD_BORDER: Color = Color::from_hex(0x2a2e3b);
    pub const TEXT_PRIMARY: Color = Color::from_hex(0xf2f4f8);
    pub const TEXT_SECONDARY: Color = Color::from_hex(0x8a92a5);
    pub const TEXT_MUTED: Color = Color::from_hex(0x575e70);
    pub const ACCENT_CYAN: Color = Color::from_hex(0x00d2ff);
    pub const ACCENT_BLUE: Color = Color::from_hex(0x0078d4);
    pub const ACCENT_GREEN: Color = Color::from_hex(0x10b981);
    pub const ACCENT_AMBER: Color = Color::from_hex(0xf59e0b);
    pub const ACCENT_RED: Color = Color::from_hex(0xef4444);
    pub const ACCENT_PURPLE: Color = Color::from_hex(0x8b5cf6);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_geometry() {
        let r = Rect::from_points(10.0, 20.0, 100.0, 50.0);
        assert_eq!(r.width(), 100.0);
        assert_eq!(r.height(), 50.0);
        assert_eq!(r.center(), Point::new(60.0, 45.0));

        let inset = r.inset(5.0, 10.0);
        assert_eq!(inset.left, 15.0);
        assert_eq!(inset.top, 30.0);
        assert_eq!(inset.right, 105.0);
        assert_eq!(inset.bottom, 60.0);
    }

    #[test]
    fn test_color_hex() {
        let c = Color::from_hex(0x00d2ff);
        assert!((c.r - 0.0).abs() < 0.01);
        assert!((c.g - (210.0 / 255.0)).abs() < 0.01);
        assert!((c.b - 1.0).abs() < 0.01);
        assert_eq!(c.a, 1.0);
    }
}
