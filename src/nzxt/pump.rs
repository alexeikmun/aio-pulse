use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PumpMode {
    Silent,
    Performance,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PumpState {
    pub current_rpm: u16,
    pub target_duty_percent: u8,
    pub mode: PumpMode,
}

impl Default for PumpState {
    fn default() -> Self {
        Self {
            current_rpm: 2400,
            target_duty_percent: 60,
            mode: PumpMode::Silent,
        }
    }
}
