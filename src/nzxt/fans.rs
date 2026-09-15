use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FanMode {
    Silent,
    Performance,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanState {
    pub current_rpm: u16,
    pub target_duty_percent: u8,
    pub mode: FanMode,
}

impl Default for FanState {
    fn default() -> Self {
        Self {
            current_rpm: 1200,
            target_duty_percent: 50,
            mode: FanMode::Silent,
        }
    }
}
