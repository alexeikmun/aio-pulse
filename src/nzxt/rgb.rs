use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RgbEffect {
    Static,
    Breathing,
    Rainbow,
    LiquidSync,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Default for RgbColor {
    fn default() -> Self {
        Self { r: 0, g: 210, b: 255 } // Default Cyan
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RgbState {
    pub effect: RgbEffect,
    pub primary_color: RgbColor,
    pub secondary_color: RgbColor,
    pub speed: u8,
    pub brightness: u8,
}

impl Default for RgbState {
    fn default() -> Self {
        Self {
            effect: RgbEffect::Static,
            primary_color: RgbColor::default(),
            secondary_color: RgbColor { r: 16, g: 185, b: 129 },
            speed: 50,
            brightness: 80,
        }
    }
}
