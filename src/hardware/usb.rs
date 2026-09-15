use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct UsbDeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: Option<String>,
    pub product_name: Option<String>,
    pub serial_number: Option<String>,
    pub interface_number: i32,
    pub path: String,
}

pub struct UsbScanner;

impl UsbScanner {
    pub fn enumerate_devices() -> Vec<UsbDeviceInfo> {
        let mut list = Vec::new();
        match hidapi::HidApi::new() {
            Ok(api) => {
                for dev in api.device_list() {
                    let info = UsbDeviceInfo {
                        vendor_id: dev.vendor_id(),
                        product_id: dev.product_id(),
                        manufacturer: dev.manufacturer_string().map(|s| s.to_string()),
                        product_name: dev.product_string().map(|s| s.to_string()),
                        serial_number: dev.serial_number().map(|s| s.to_string()),
                        interface_number: dev.interface_number(),
                        path: dev.path().to_string_lossy().to_string(),
                    };
                    list.push(info);
                }
            }
            Err(err) => {
                debug!("Could not initialize HidApi for enumeration: {}", err);
            }
        }
        list
    }

    pub fn find_by_vid_pid(vid: u16, pids: &[u16]) -> Vec<UsbDeviceInfo> {
        let all = Self::enumerate_devices();
        let matched: Vec<UsbDeviceInfo> = all
            .into_iter()
            .filter(|d| d.vendor_id == vid && pids.contains(&d.product_id))
            .collect();

        if !matched.is_empty() {
            info!(
                "Detected {} matching USB device(s) for VID 0x{:04x}",
                matched.len(),
                vid
            );
            for d in &matched {
                info!(
                    "  -> PID: 0x{:04x} | {} | Serial: {:?}",
                    d.product_id,
                    d.product_name.as_deref().unwrap_or("Unknown"),
                    d.serial_number
                );
            }
        }
        matched
    }
}
