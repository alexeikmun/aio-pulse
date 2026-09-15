use crate::hardware::device::{
    DeviceConnectionStatus, HardwareDevice, HardwareError, HardwareTelemetry,
};
use crate::hardware::hid::HidTransport;
use crate::hardware::usb::{UsbDeviceInfo, UsbScanner};
use std::time::Instant;
use tracing::{info, warn};

pub const NZXT_VID: u16 = 0x1E71;

pub const KRAKEN_PIDS: &[u16] = &[
    0x1707, // Kraken X52/X62/X72
    0x2007, // Kraken X53/X63/X73
    0x3008, // Kraken Z53/Z63/Z73 (320x320 LCD)
    0x300c, // Kraken 2023 (240x240 LCD)
    0x300e, // Kraken Elite 2023 (640x640 LCD)
];

pub struct KrakenDevice {
    name: String,
    status: DeviceConnectionStatus,
    transport: HidTransport,
    device_info: Option<UsbDeviceInfo>,
    firmware_version: String,
    sim_start: Instant,
}

impl KrakenDevice {
    pub fn new() -> Self {
        Self {
            name: "NZXT Kraken 360".to_string(),
            status: DeviceConnectionStatus::Disconnected,
            transport: HidTransport::new(),
            device_info: None,
            firmware_version: "Unknown".to_string(),
            sim_start: Instant::now(),
        }
    }

    fn identify_model(pid: u16) -> &'static str {
        match pid {
            0x1707 => "NZXT Kraken X62/X72",
            0x2007 => "NZXT Kraken X53/X63/X73",
            0x3008 => "NZXT Kraken Z73",
            0x300c => "NZXT Kraken 360",
            0x300e => "NZXT Kraken 2023 (Base)",
            _ => "NZXT Kraken AIO",
        }
    }
}

impl Default for KrakenDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareDevice for KrakenDevice {
    fn device_name(&self) -> &str {
        &self.name
    }

    fn connection_status(&self) -> DeviceConnectionStatus {
        self.status
    }

    fn connect(&mut self) -> Result<(), HardwareError> {
        info!("Scanning for NZXT Kraken hardware (VID 0x{:04x})...", NZXT_VID);
        let found = UsbScanner::find_by_vid_pid(NZXT_VID, KRAKEN_PIDS);

        // For composite devices with HID on interface 1, prefer interface 1
        let target_dev = found
            .iter()
            .find(|d| d.interface_number == 1)
            .or_else(|| found.first())
            .cloned();

        if let Some(dev) = target_dev {
            let model_name = Self::identify_model(dev.product_id);
            self.name = model_name.to_string();
            info!(
                "Identified {}: PID 0x{:04x}, Interface {}, Serial {:?}",
                self.name, dev.product_id, dev.interface_number, dev.serial_number
            );

            match self.transport.open_path(&dev.path) {
                Ok(()) => {
                    self.status = DeviceConnectionStatus::Connected;
                    self.device_info = Some(dev);

                    // Query firmware version
                    let mut fw_cmd = [0u8; 64];
                    fw_cmd[0] = 0x10;
                    fw_cmd[1] = 0x01;
                    let _ = self.transport.write_safe(&fw_cmd);

                    let mut read_buf = [0u8; 64];
                    if let Ok(n) = self.transport.read_timeout(&mut read_buf, 300) {
                        if n >= 20 && read_buf[0] == 0x11 && read_buf[1] == 0x01 {
                            self.firmware_version = format!(
                                "{}.{}.{}",
                                read_buf[0x11], read_buf[0x12], read_buf[0x13]
                            );
                            info!("Kraken firmware version: {}", self.firmware_version);
                        }
                    }

                    info!("Successfully attached to physical {}", self.name);
                    return Ok(());
                }
                Err(err) => {
                    warn!(
                        "Could not open exclusive access to {} (may be in use): {}. Falling back to Simulated mode.",
                        self.name, err
                    );
                    self.status = DeviceConnectionStatus::Simulated;
                    self.device_info = Some(dev);
                    return Ok(());
                }
            }
        }

        info!("No physical NZXT Kraken detected. Activating Simulated mode.");
        self.status = DeviceConnectionStatus::Simulated;
        Ok(())
    }

    fn disconnect(&mut self) {
        self.transport.close();
        self.status = DeviceConnectionStatus::Disconnected;
        self.device_info = None;
        info!("Disconnected from {}", self.name);
    }

    fn poll_telemetry(&mut self) -> Result<HardwareTelemetry, HardwareError> {
        match self.status {
            DeviceConnectionStatus::Disconnected => Err(HardwareError::NotConnected),
            DeviceConnectionStatus::Connected => {
                let mut buf = [0u8; 64];
                let mut liquid = 32.0;
                let mut pump = 2200;
                let mut fans = 750;

                // Read report 0x75 0x02
                if let Ok(n) = self.transport.read_timeout(&mut buf, 100) {
                    if n >= 25 && buf[0] == 0x75 && buf[1] == 0x02 {
                        let l_int = buf[15] as f32;
                        let l_frac = buf[16] as f32 / 10.0;
                        liquid = l_int + l_frac;
                        pump = u16::from_le_bytes([buf[17], buf[18]]);
                        fans = u16::from_le_bytes([buf[23], buf[24]]);
                    }
                }

                Ok(HardwareTelemetry {
                    liquid_temperature: liquid,
                    pump_rpm: pump,
                    fan_rpm: fans,
                    firmware_version: if self.firmware_version != "Unknown" {
                        self.firmware_version.clone()
                    } else {
                        "2.0.0".to_string()
                    },
                    serial_number: self
                        .device_info
                        .as_ref()
                        .and_then(|d| d.serial_number.clone())
                        .unwrap_or_else(|| "NZXT-KRK-360-SN001".to_string()),
                })
            }
            DeviceConnectionStatus::Simulated => {
                let elapsed = self.sim_start.elapsed().as_secs_f64();
                let liquid = 31.8 + (elapsed * 0.1).sin() as f32 * 1.8;
                let pump = (2450.0 + (elapsed * 0.2).cos() * 90.0) as u16;
                let fans = (1180.0 + (elapsed * 0.15).sin() * 60.0) as u16;

                Ok(HardwareTelemetry {
                    liquid_temperature: liquid,
                    pump_rpm: pump,
                    fan_rpm: fans,
                    firmware_version: "Sim-1.0".to_string(),
                    serial_number: "SIMULATED-KRAKEN-360".to_string(),
                })
            }
        }
    }
}
