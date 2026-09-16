use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{error, info};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use super::state::AppState;
use crate::config::AppConfig;
use crate::hardware::device::HardwareDevice;
use crate::monitoring::{CpuMonitor, GpuMonitor, MemoryMonitor, SensorProvider};
use crate::nzxt::kraken::KrakenDevice;
use crate::nzxt::lcd::{Display, InfographicRotationManager, InfographicType, KrakenLcd, LcdFramebuffer};
use crate::rendering::{D2DOffscreenLcd, GpuCpuMetrics};

pub const WM_APP_UPDATE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 2;

pub struct Application {
    pub state: Arc<RwLock<AppState>>,
    pub running: Arc<AtomicBool>,
}

impl Application {
    pub fn new(config: AppConfig) -> Self {
        let state = Arc::new(RwLock::new(AppState::new(config)));
        let running = Arc::new(AtomicBool::new(true));

        Self { state, running }
    }

    pub fn start_workers(&self, hwnd: HWND) {
        let hwnd_raw = hwnd.0 as usize;

        // 1. Hardware Communication Thread
        {
            let state_clone = Arc::clone(&self.state);
            let running_clone = Arc::clone(&self.running);

            thread::Builder::new()
                .name("AIPulse-Hardware".into())
                .spawn(move || {
                    info!("Hardware worker thread started.");
                    let mut kraken = KrakenDevice::new();
                    if let Err(err) = kraken.connect() {
                        error!("Initial hardware connect error: {}", err);
                    }

                    while running_clone.load(Ordering::Relaxed) {
                        match kraken.poll_telemetry() {
                            Ok(telemetry) => {
                                if let Ok(mut state) = state_clone.write() {
                                    state.kraken.device_name = kraken.device_name().to_string();
                                    state.kraken.status = kraken.connection_status();
                                    state.kraken.liquid_temperature = telemetry.liquid_temperature;
                                    state.kraken.pump_rpm = telemetry.pump_rpm;
                                    state.kraken.fan_rpm = telemetry.fan_rpm;
                                    state.kraken.firmware_version = telemetry.firmware_version;
                                    state.kraken.serial_number = telemetry.serial_number;
                                }
                            }
                            Err(err) => {
                                error!("Hardware poll error: {}. Attempting reconnect...", err);
                                let _ = kraken.connect();
                            }
                        }

                        thread::sleep(Duration::from_millis(1000));
                    }
                    kraken.disconnect();
                    info!("Hardware worker thread exiting.");
                })
                .expect("Failed to spawn hardware worker thread");
        }

        // 2. Monitoring Sensors Thread (1-second monitoring interval)
        {
            let state_clone = Arc::clone(&self.state);
            let running_clone = Arc::clone(&self.running);

            thread::Builder::new()
                .name("AIPulse-Monitoring".into())
                .spawn(move || {
                    info!("System monitoring worker thread started (1-second interval).");
                    let mut cpu_mon = CpuMonitor::new();
                    let mut gpu_mon = GpuMonitor::new();
                    let mut mem_mon = MemoryMonitor::new();

                    if let Some(gpu_name) = gpu_mon.device_name() {
                        if let Ok(mut state) = state_clone.write() {
                            state.sensors.gpu_name = gpu_name.to_string();
                        }
                    }

                    let mut next_poll = Instant::now();
                    while running_clone.load(Ordering::Relaxed) {
                        next_poll += Duration::from_secs(1);

                        let cpu_sensors = cpu_mon.poll_sensors();
                        let gpu_sensors = gpu_mon.poll_sensors();
                        let mem_sensors = mem_mon.poll_sensors();

                        if let Ok(mut state) = state_clone.write() {
                            for s in cpu_sensors {
                                match s.name.as_str() {
                                    "CPU Temperature" => state.sensors.cpu_temperature = s.value as f32,
                                    "CPU Usage" => state.sensors.cpu_usage = s.value as f32,
                                    "CPU Clock" => state.sensors.cpu_clock_mhz = s.value as f32,
                                    _ => {}
                                }
                            }

                            for s in gpu_sensors {
                                match s.name.as_str() {
                                    "GPU Temperature" => state.sensors.gpu_temperature = s.value as f32,
                                    "GPU Usage" => state.sensors.gpu_usage = s.value as f32,
                                    "GPU VRAM" => state.sensors.gpu_vram_mb = s.value as f32,
                                    _ => {}
                                }
                            }

                            for s in mem_sensors {
                                if s.name == "RAM Usage" {
                                    state.sensors.ram_usage = s.value as f32;
                                }
                            }
                            state.sensors.update_count = state.sensors.update_count.wrapping_add(1);
                        }

                        // Also notify main window to refresh telemetry cards immediately
                        unsafe {
                            let h = HWND(hwnd_raw as *mut _);
                            let _ = PostMessageW(Some(h), WM_APP_UPDATE, WPARAM(0), LPARAM(0));
                        }

                        let now = Instant::now();
                        if next_poll > now {
                            thread::sleep(next_poll - now);
                        } else {
                            next_poll = now;
                        }
                    }
                    info!("System monitoring worker thread exiting.");
                })
                .expect("Failed to spawn monitoring worker thread");
        }

        // 3. LCD Rendering Worker Thread (5-second infographic rotation cycle, 1-second value updates)
        {
            let state_clone = Arc::clone(&self.state);
            let running_clone = Arc::clone(&self.running);

            thread::Builder::new()
                .name("AIPulse-LCD".into())
                .spawn(move || {
                    info!("LCD worker thread started (5-second rotation cycle, 1-second value updates, Direct2D pipeline).");
                    let width = 240;
                    let height = 240;
                    let mut kraken_lcd = KrakenLcd::new(width, height);

                    if let Err(e) = kraken_lcd.connect() {
                        info!("Physical Kraken LCD connection deferred: {}", e);
                    }

                    // Direct2D offscreen render target for physical LCD hardware streaming
                    let mut offscreen_lcd = D2DOffscreenLcd::new(width, height).ok();
                    let mut fallback_framebuffer = LcdFramebuffer::new(width, height);

                    let mut rotation_mgr = InfographicRotationManager::new(InfographicType::CpuGpuLoad);
                    let mut last_sensor_render_time = Instant::now() - Duration::from_secs(10);
                    let mut last_rendered_infographic = rotation_mgr.current();
                    let mut last_rendered_sensor_count = 0u64;
                    let sensor_refresh_interval = Duration::from_secs(1);

                    while running_clone.load(Ordering::Relaxed) {
                        let now = Instant::now();

                        // 1. Sync external active_infographic and sensor update version
                        let (current_from_state, current_sensor_count) = {
                            if let Ok(state) = state_clone.read() {
                                (state.lcd.active_infographic, state.sensors.update_count)
                            } else {
                                (rotation_mgr.current(), last_rendered_sensor_count)
                            }
                        };

                        // If infographic was manually changed externally, sync and reset 5-second timer
                        if current_from_state != rotation_mgr.current() {
                            rotation_mgr.set_current(current_from_state, now);
                        }

                        // 2. Check 5-second rotation timer: switches automatically when 5s elapsed
                        let rotated = rotation_mgr.tick(now);
                        let active_infographic = rotation_mgr.current();

                        // 3. Check if re-render is needed:
                        // - Infographic switched (5s boundary)
                        // - New sensor telemetry sampled (every 1 second)
                        // - 1-second fallback sensor refresh interval elapsed
                        let new_sensor_data = current_sensor_count != last_rendered_sensor_count;
                        let sensor_refresh_due = now.duration_since(last_sensor_render_time) >= sensor_refresh_interval;
                        let infographic_changed = active_infographic != last_rendered_infographic;

                        if rotated || infographic_changed || sensor_refresh_due || new_sensor_data {
                            // Snapshot sensor telemetry
                            let (metrics, cpu_temp, gpu_temp, liquid_temp) = {
                                if let Ok(state) = state_clone.read() {
                                    (
                                        GpuCpuMetrics::new(
                                            state.sensors.cpu_usage,
                                            state.sensors.gpu_usage,
                                        ),
                                        state.sensors.cpu_temperature,
                                        state.sensors.gpu_temperature,
                                        state.kraken.liquid_temperature,
                                    )
                                } else {
                                    (GpuCpuMetrics::default(), 42.0, 56.0, 32.0)
                                }
                            };

                            // Render frame for active infographic via Direct2D (with fallback to LcdFramebuffer)
                            let rendered_slice: &[u8] = if let Some(ref mut d2d) = offscreen_lcd {
                                match d2d.render_infographic_frame(
                                    active_infographic,
                                    &metrics,
                                    cpu_temp,
                                    gpu_temp,
                                    liquid_temp,
                                ) {
                                    Ok(buf) => buf,
                                    Err(e) => {
                                        error!("Direct2D offscreen render error: {}. Using fallback.", e);
                                        fallback_framebuffer.render_active_infographic(
                                            active_infographic,
                                            metrics.cpu_usage,
                                            metrics.gpu_usage,
                                            cpu_temp,
                                            gpu_temp,
                                            liquid_temp,
                                        );
                                        &fallback_framebuffer.pixels
                                    }
                                }
                            } else {
                                fallback_framebuffer.render_active_infographic(
                                    active_infographic,
                                    metrics.cpu_usage,
                                    metrics.gpu_usage,
                                    cpu_temp,
                                    gpu_temp,
                                    liquid_temp,
                                );
                                &fallback_framebuffer.pixels
                            };

                            // Update application state preview buffer and active infographic
                            if let Ok(mut state) = state_clone.write() {
                                state.lcd.width = width;
                                state.lcd.height = height;
                                state.lcd.active_infographic = active_infographic;
                                state.lcd.preview_buffer.copy_from_slice(rendered_slice);
                            }

                            // Stream to physical Kraken 360 LCD
                            let _ = kraken_lcd.submit_frame(rendered_slice);

                            // Request LCD redraw in desktop UI
                            unsafe {
                                let h = HWND(hwnd_raw as *mut _);
                                let _ = PostMessageW(Some(h), WM_APP_UPDATE, WPARAM(0), LPARAM(0));
                            }

                            if new_sensor_data || sensor_refresh_due {
                                last_rendered_sensor_count = current_sensor_count;
                                last_sensor_render_time = now;
                            }
                            last_rendered_infographic = active_infographic;
                        }

                        // High-resolution sleep to ensure deterministic 5-second switching
                        thread::sleep(Duration::from_millis(50));
                    }
                    info!("LCD worker thread exiting.");
                })
                .expect("Failed to spawn LCD worker thread");
        }
    }

    pub fn shutdown(&self) {
        info!("Shutting down application workers...");
        self.running.store(false, Ordering::SeqCst);
    }
}
