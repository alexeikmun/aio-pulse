use crate::config::AppConfig;
use crate::hardware::device::DeviceConnectionStatus;
use crate::nzxt::lcd::InfographicType;
use crate::nzxt::rgb::RgbState;

#[derive(Debug, Clone)]
pub struct KrakenState {
    pub device_name: String,
    pub status: DeviceConnectionStatus,
    pub liquid_temperature: f32,
    pub pump_rpm: u16,
    pub fan_rpm: u16,
    pub serial_number: String,
    pub firmware_version: String,
}

impl Default for KrakenState {
    fn default() -> Self {
        Self {
            device_name: "NZXT Kraken 360".to_string(),
            status: DeviceConnectionStatus::Disconnected,
            liquid_temperature: 32.0,
            pump_rpm: 2450,
            fan_rpm: 1200,
            serial_number: "SIMULATED-KRAKEN-360".to_string(),
            firmware_version: "v1.2.4".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SensorState {
    pub cpu_temperature: f32,
    pub cpu_usage: f32,
    pub cpu_clock_mhz: f32,
    pub gpu_name: String,
    pub gpu_temperature: f32,
    pub gpu_usage: f32,
    pub gpu_vram_mb: f32,
    pub ram_usage: f32,
}

impl Default for SensorState {
    fn default() -> Self {
        Self {
            cpu_temperature: 42.0,
            cpu_usage: 24.0,
            cpu_clock_mhz: 4600.0,
            gpu_name: "NVIDIA GeForce RTX 5080".to_string(),
            gpu_temperature: 56.0,
            gpu_usage: 62.0,
            gpu_vram_mb: 6144.0,
            ram_usage: 44.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LcdState {
    pub width: u32,
    pub height: u32,
    pub brightness: u8,
    pub active_infographic: InfographicType,
    pub is_clearing: bool,
    pub preview_buffer: Vec<u8>,
}

impl Default for LcdState {
    fn default() -> Self {
        let width = 240;
        let height = 240;
        let size = (width * height * 4) as usize;
        Self {
            width,
            height,
            brightness: 80,
            active_infographic: InfographicType::CpuGpuLoad,
            is_clearing: false,
            preview_buffer: vec![0u8; size],
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub kraken: KrakenState,
    pub sensors: SensorState,
    pub lcd: LcdState,
    pub rgb: RgbState,
    pub config: AppConfig,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        Self {
            kraken: KrakenState::default(),
            sensors: SensorState::default(),
            lcd: LcdState::default(),
            rgb: RgbState::default(),
            config,
        }
    }
}
