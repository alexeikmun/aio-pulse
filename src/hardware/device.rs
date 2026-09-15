use thiserror::Error;

#[derive(Debug, Error)]
pub enum HardwareError {
    #[error("Device not found")]
    DeviceNotFound,
    #[error("Access denied or device in use by another process (e.g. CAM): {0}")]
    AccessDenied(String),
    #[error("Communication error: {0}")]
    Communication(String),
    #[error("Device is disconnected")]
    NotConnected,
    #[error("Unsupported operation: {0}")]
    Unsupported(String),
}

#[derive(Debug, Clone, Default)]
pub struct HardwareTelemetry {
    pub liquid_temperature: f32,
    pub pump_rpm: u16,
    pub fan_rpm: u16,
    pub firmware_version: String,
    pub serial_number: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceConnectionStatus {
    Disconnected,
    Connected,
    Simulated,
}

pub trait HardwareDevice: Send {
    fn device_name(&self) -> &str;
    fn connection_status(&self) -> DeviceConnectionStatus;
    fn connect(&mut self) -> Result<(), HardwareError>;
    fn disconnect(&mut self);
    fn is_connected(&self) -> bool {
        matches!(
            self.connection_status(),
            DeviceConnectionStatus::Connected | DeviceConnectionStatus::Simulated
        )
    }
    fn poll_telemetry(&mut self) -> Result<HardwareTelemetry, HardwareError>;
}
