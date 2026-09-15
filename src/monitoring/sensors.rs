use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SensorUnit {
    Celsius,
    Percent,
    Rpm,
    Megahertz,
    Megabytes,
    Watts,
    Fps,
}

impl SensorUnit {
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Celsius => "°",
            Self::Percent => "%",
            Self::Rpm => "RPM",
            Self::Megahertz => "MHz",
            Self::Megabytes => "MB",
            Self::Watts => "W",
            Self::Fps => "FPS",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SensorValue {
    pub name: String,
    pub value: f64,
    pub unit: SensorUnit,
    pub timestamp: Instant,
}

impl SensorValue {
    pub fn new(name: impl Into<String>, value: f64, unit: SensorUnit) -> Self {
        Self {
            name: name.into(),
            value,
            unit,
            timestamp: Instant::now(),
        }
    }
}

pub trait SensorProvider: Send + Sync {
    fn name(&self) -> &str;
    fn poll_sensors(&mut self) -> Vec<SensorValue>;
}
