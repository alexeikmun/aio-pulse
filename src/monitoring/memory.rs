use super::sensors::{SensorProvider, SensorUnit, SensorValue};
use std::time::Instant;

pub struct MemoryMonitor {
    start_time: Instant,
}

impl MemoryMonitor {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }
}

impl Default for MemoryMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl SensorProvider for MemoryMonitor {
    fn name(&self) -> &str {
        "Memory Monitor"
    }

    fn poll_sensors(&mut self) -> Vec<SensorValue> {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let base_usage = 42.0 + (elapsed * 0.1).sin() * 5.0;

        vec![
            SensorValue::new("RAM Usage", base_usage.clamp(10.0, 95.0), SensorUnit::Percent),
        ]
    }
}
