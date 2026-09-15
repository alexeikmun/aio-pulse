use super::device::HardwareError;
use hidapi::{HidApi, HidDevice};
use std::ffi::CString;
use tracing::{debug, error, info};

pub struct HidTransport {
    device: Option<HidDevice>,
    path: Option<String>,
}

impl HidTransport {
    pub fn new() -> Self {
        Self {
            device: None,
            path: None,
        }
    }

    pub fn open_path(&mut self, path: &str) -> Result<(), HardwareError> {
        let api = HidApi::new().map_err(|e| HardwareError::Communication(e.to_string()))?;
        let c_path = CString::new(path).map_err(|_| HardwareError::Communication("Invalid path".into()))?;

        match api.open_path(&c_path) {
            Ok(dev) => {
                info!("Successfully opened HID device at {}", path);
                self.device = Some(dev);
                self.path = Some(path.to_string());
                Ok(())
            }
            Err(err) => {
                error!("Failed to open HID device at {}: {}", path, err);
                Err(HardwareError::AccessDenied(err.to_string()))
            }
        }
    }

    pub fn is_open(&self) -> bool {
        self.device.is_some()
    }

    pub fn close(&mut self) {
        if self.device.is_some() {
            debug!("Closing HID device");
            self.device = None;
            self.path = None;
        }
    }

    pub fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize, HardwareError> {
        if let Some(ref dev) = self.device {
            match dev.read_timeout(buf, timeout_ms) {
                Ok(bytes_read) => Ok(bytes_read),
                Err(err) => Err(HardwareError::Communication(err.to_string())),
            }
        } else {
            Err(HardwareError::NotConnected)
        }
    }

    pub fn write_safe(&self, packet: &[u8]) -> Result<usize, HardwareError> {
        if let Some(ref dev) = self.device {
            match dev.write(packet) {
                Ok(bytes_written) => Ok(bytes_written),
                Err(err) => Err(HardwareError::Communication(err.to_string())),
            }
        } else {
            Err(HardwareError::NotConnected)
        }
    }
}

impl Default for HidTransport {
    fn default() -> Self {
        Self::new()
    }
}
