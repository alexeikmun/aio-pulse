use super::sensors::{SensorProvider, SensorUnit, SensorValue};
use std::ffi::{c_char, c_void, CStr};
use std::time::Instant;
use tracing::{info, warn};
use windows::core::{s, w};
use windows::Win32::Foundation::{FreeLibrary, HMODULE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NvmlUtilization {
    pub gpu: u32,
    pub memory: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct NvmlMemory {
    pub total: u64,
    pub free: u64,
    pub used: u64,
}

type NvmlInitV2 = unsafe extern "C" fn() -> i32;
type NvmlShutdown = unsafe extern "C" fn() -> i32;
type NvmlDeviceGetCountV2 = unsafe extern "C" fn(*mut u32) -> i32;
type NvmlDeviceGetHandleByIndexV2 = unsafe extern "C" fn(u32, *mut *mut c_void) -> i32;
type NvmlDeviceGetName = unsafe extern "C" fn(*mut c_void, *mut c_char, u32) -> i32;
type NvmlDeviceGetTemperature = unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> i32;
type NvmlDeviceGetUtilizationRates = unsafe extern "C" fn(*mut c_void, *mut NvmlUtilization) -> i32;
type NvmlDeviceGetMemoryInfo = unsafe extern "C" fn(*mut c_void, *mut NvmlMemory) -> i32;

struct NvmlContext {
    module: HMODULE,
    fn_shutdown: NvmlShutdown,
    fn_get_temp: NvmlDeviceGetTemperature,
    fn_get_util: NvmlDeviceGetUtilizationRates,
    fn_get_mem: NvmlDeviceGetMemoryInfo,
    device_handle: *mut c_void,
    pub device_name: String,
}

unsafe impl Send for NvmlContext {}
unsafe impl Sync for NvmlContext {}

impl Drop for NvmlContext {
    fn drop(&mut self) {
        unsafe {
            (self.fn_shutdown)();
            let _ = FreeLibrary(self.module);
        }
    }
}

impl NvmlContext {
    fn load() -> Option<Self> {
        unsafe {
            let module = LoadLibraryW(w!("nvml.dll")).ok()?;

            macro_rules! resolve {
                ($fn_type:ty, $name:expr) => {
                    match GetProcAddress(module, $name) {
                        Some(p) => std::mem::transmute::<_, $fn_type>(p),
                        None => {
                            let _ = FreeLibrary(module);
                            return None;
                        }
                    }
                };
            }

            let fn_init: NvmlInitV2 = resolve!(NvmlInitV2, s!("nvmlInit_v2"));
            let fn_shutdown: NvmlShutdown = resolve!(NvmlShutdown, s!("nvmlShutdown"));
            let fn_get_count: NvmlDeviceGetCountV2 = resolve!(NvmlDeviceGetCountV2, s!("nvmlDeviceGetCount_v2"));
            let fn_get_handle: NvmlDeviceGetHandleByIndexV2 = resolve!(NvmlDeviceGetHandleByIndexV2, s!("nvmlDeviceGetHandleByIndex_v2"));
            let fn_get_name: NvmlDeviceGetName = resolve!(NvmlDeviceGetName, s!("nvmlDeviceGetName"));
            let fn_get_temp: NvmlDeviceGetTemperature = resolve!(NvmlDeviceGetTemperature, s!("nvmlDeviceGetTemperature"));
            let fn_get_util: NvmlDeviceGetUtilizationRates = resolve!(NvmlDeviceGetUtilizationRates, s!("nvmlDeviceGetUtilizationRates"));
            let fn_get_mem: NvmlDeviceGetMemoryInfo = resolve!(NvmlDeviceGetMemoryInfo, s!("nvmlDeviceGetMemoryInfo"));

            if fn_init() != 0 {
                let _ = FreeLibrary(module);
                return None;
            }

            let mut count = 0u32;
            if fn_get_count(&mut count) != 0 || count == 0 {
                fn_shutdown();
                let _ = FreeLibrary(module);
                return None;
            }

            let mut selected_handle = std::ptr::null_mut();
            let mut selected_name = String::new();

            for i in 0..count {
                let mut handle = std::ptr::null_mut();
                if fn_get_handle(i, &mut handle) == 0 {
                    let mut name_buf = [0u8; 96];
                    if fn_get_name(handle, name_buf.as_mut_ptr() as *mut c_char, 96) == 0 {
                        let name = CStr::from_ptr(name_buf.as_ptr() as *const c_char)
                            .to_string_lossy()
                            .into_owned();
                        info!("Found NVML GPU {}: {}", i, name);

                        // If RTX 5080 found, prioritize it
                        if name.contains("5080") {
                            selected_handle = handle;
                            selected_name = name;
                            break;
                        }

                        if selected_handle.is_null() {
                            selected_handle = handle;
                            selected_name = name;
                        }
                    }
                }
            }

            if selected_handle.is_null() {
                fn_shutdown();
                let _ = FreeLibrary(module);
                return None;
            }

            info!("Active NVML GPU monitoring selected: {}", selected_name);

            Some(Self {
                module,
                fn_shutdown,
                fn_get_temp,
                fn_get_util,
                fn_get_mem,
                device_handle: selected_handle,
                device_name: selected_name,
            })
        }
    }
}

pub struct GpuMonitor {
    nvml: Option<NvmlContext>,
    start_time: Instant,
}

impl GpuMonitor {
    pub fn new() -> Self {
        let nvml = NvmlContext::load();
        if nvml.is_none() {
            warn!("NVML unavailable. Falling back to simulated GPU metrics.");
        }
        Self {
            nvml,
            start_time: Instant::now(),
        }
    }

    pub fn device_name(&self) -> Option<&str> {
        self.nvml.as_ref().map(|n| n.device_name.as_str())
    }
}

impl Default for GpuMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl SensorProvider for GpuMonitor {
    fn name(&self) -> &str {
        "GPU Monitor"
    }

    fn poll_sensors(&mut self) -> Vec<SensorValue> {
        if let Some(nvml) = &self.nvml {
            unsafe {
                let mut temp = 0u32;
                let mut util = NvmlUtilization::default();
                let mut mem = NvmlMemory::default();

                let temp_ok = (nvml.fn_get_temp)(nvml.device_handle, 0, &mut temp) == 0;
                let util_ok = (nvml.fn_get_util)(nvml.device_handle, &mut util) == 0;
                let mem_ok = (nvml.fn_get_mem)(nvml.device_handle, &mut mem) == 0;

                let temp_c = if temp_ok { temp as f64 } else { 0.0 };
                let usage_pct = if util_ok { util.gpu as f64 } else { 0.0 };
                let vram_mb = if mem_ok { (mem.used as f64) / (1024.0 * 1024.0) } else { 0.0 };

                vec![
                    SensorValue::new("GPU Temperature", temp_c, SensorUnit::Celsius),
                    SensorValue::new("GPU Usage", usage_pct, SensorUnit::Percent),
                    SensorValue::new("GPU VRAM", vram_mb, SensorUnit::Megabytes),
                ]
            }
        } else {
            let elapsed = self.start_time.elapsed().as_secs_f64();
            let base_temp = 54.0 + (elapsed * 0.25).sin() * 8.0;
            let base_usage = 58.0 + (elapsed * 0.4).cos() * 20.0;
            let vram_mb = 6400.0 + (elapsed * 0.15).sin() * 1200.0;

            vec![
                SensorValue::new("GPU Temperature", base_temp.clamp(30.0, 90.0), SensorUnit::Celsius),
                SensorValue::new("GPU Usage", base_usage.clamp(2.0, 99.0), SensorUnit::Percent),
                SensorValue::new("GPU VRAM", vram_mb.clamp(1024.0, 16384.0), SensorUnit::Megabytes),
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_monitor_initialization() {
        let mut monitor = GpuMonitor::new();
        let sensors = monitor.poll_sensors();
        assert_eq!(sensors.len(), 3);

        let temp = sensors.iter().find(|s| s.name == "GPU Temperature").unwrap();
        let usage = sensors.iter().find(|s| s.name == "GPU Usage").unwrap();
        let vram = sensors.iter().find(|s| s.name == "GPU VRAM").unwrap();

        assert!(temp.value >= 0.0 && temp.value <= 110.0, "Temp out of range: {}", temp.value);
        assert!(usage.value >= 0.0 && usage.value <= 100.0, "Usage out of range: {}", usage.value);
        assert!(vram.value >= 0.0, "VRAM negative: {}", vram.value);

        if let Some(name) = monitor.device_name() {
            println!("Detected GPU: {}", name);
            println!("Real Telemetry -> Temp: {:.1}°, Load: {:.1}%, VRAM: {:.1} MB", temp.value, usage.value, vram.value);
            assert!(name.contains("RTX 5080") || name.contains("NVIDIA"));
        }
    }
}
