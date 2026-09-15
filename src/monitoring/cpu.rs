use super::sensors::{SensorProvider, SensorUnit, SensorValue};
use std::time::Instant;

pub struct CpuMonitor {
    start_time: Instant,
}

impl CpuMonitor {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }
}

impl Default for CpuMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl SensorProvider for CpuMonitor {
    fn name(&self) -> &str {
        "CPU Monitor"
    }

    fn poll_sensors(&mut self) -> Vec<SensorValue> {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        // Dynamic simulated wave with slight pseudo-random jitter
        let base_temp = 44.0 + (elapsed * 0.3).sin() * 7.0;
        let base_usage = 26.0 + (elapsed * 0.5).cos() * 15.0;
        let base_clock = 4400.0 + (elapsed * 0.2).sin() * 400.0;

        vec![
            SensorValue::new("CPU Temperature", base_temp.clamp(35.0, 85.0), SensorUnit::Celsius),
            SensorValue::new("CPU Usage", base_usage.clamp(5.0, 95.0), SensorUnit::Percent),
            SensorValue::new("CPU Clock", base_clock.clamp(3200.0, 5200.0), SensorUnit::Megahertz),
        ]
    }
}
