# AIPulse ⚡

A lightweight, ultra-high-performance native Windows desktop application for controlling and monitoring the NZXT Kraken 360 AIO liquid cooler.

Built as a native alternative to NZXT CAM:
- **0.5 MB** executable size (vs ~400 MB for CAM).
- **~0% idle CPU usage** with event-driven Win32 message pump (`GetMessageW`).
- **Zero Web Technologies**: No Electron, no WebView, no Chromium, no JavaScript runtime, no .NET runtime.
- **Hardware-Accelerated**: Direct2D for 2D graphics, DirectWrite for typography.
- **DPI-Aware**: Full Per-Monitor V2 scaling for Windows 10 & 11.
- **Safe Hardware Probing**: Safe USB/HID enumeration targeting NZXT VID `0x1E71`, zero blind packet transmissions, safe fallback to simulated telemetry.
- **LCD Subsystem**: Framebuffer rendering and real-time circular preview widget in the native dashboard.
- **System Tray**: Native Windows system tray integration (`Shell_NotifyIconW`).

---

## Architecture Overview

```
aio-pulse/
├── Cargo.toml
├── src/
│   ├── main.rs                 # WinMain entry flow, logging init, subsystem launch
│   ├── app/
│   │   ├── application.rs      # Coordinator for Hardware, Monitoring, and LCD worker threads
│   │   ├── state.rs            # Thread-safe AppState with immutable render snapshotting
│   │   └── mod.rs
│   ├── windows/
│   │   ├── window.rs           # Native Win32 window (HWND), WndProc, dark title bar, Direct2D UI
│   │   ├── message_loop.rs     # Pure event-driven Win32 message loop (zero busy-wait)
│   │   ├── tray.rs             # Native Windows system tray integration
│   │   ├── dpi.rs              # Per-Monitor V2 DPI awareness and coordinate scaling
│   │   └── mod.rs
│   ├── rendering/
│   │   ├── renderer.rs         # High-level Renderer trait abstraction
│   │   ├── direct2d.rs         # Direct2D context, brush caching, D2DERR_RECREATE_TARGET recovery
│   │   ├── directwrite.rs      # DirectWrite context, cached font formats and typography roles
│   │   ├── primitives.rs       # Geometry primitives (Rect, Point, Size, Color)
│   │   └── mod.rs
│   ├── hardware/
│   │   ├── device.rs           # HardwareDevice trait, connection status, telemetry structs
│   │   ├── usb.rs              # Safe USB device scanner and VID/PID matching
│   │   ├── hid.rs              # Safe HID transport wrapper with read timeouts
│   │   └── mod.rs
│   ├── nzxt/
│   │   ├── kraken.rs           # KrakenDevice (NZXT VID 0x1E71, Z-series / 2023 models)
│   │   ├── lcd.rs              # LCD subsystem: Display trait, LcdFramebuffer, SimulatedDisplay
│   │   ├── pump.rs             # Pump profile & control models
│   │   ├── fans.rs             # Fan curves & speed models
│   │   ├── rgb.rs              # RGB channel models & lighting effects
│   │   └── mod.rs
│   ├── monitoring/
│   │   ├── sensors.rs          # SensorValue, SensorUnit, SensorProvider trait
│   │   ├── cpu.rs              # CPU temperature, usage %, and clock speed monitor
│   │   ├── gpu.rs              # GPU temperature, usage %, and VRAM monitor
│   │   ├── memory.rs           # RAM usage monitor
│   │   └── mod.rs
│   └── config/
│       ├── settings.rs         # Versioned JSON settings persisted to %APPDATA%\AIPulse
│       └── mod.rs
```

---

## Building & Running

### Requirements
- Windows 10 (1809+) or Windows 11
- Rust 1.82+ (MSVC toolchain recommended)

### Build Debug
```pwsh
cargo build
```

### Build Release (Optimized, ~568 KB)
```pwsh
cargo build --release
```

### Run
```pwsh
cargo run --release
```
Or launch `target/release/aio-pulse.exe` directly.

### Run Tests
```pwsh
cargo test
```

---

## Roadmap / Future Phases

- **Phase 1 (Complete)**: Native Windows 11 application foundation, Direct2D/DirectWrite rendering abstraction, thread-safe state, simulated sensor monitoring, live circular LCD preview widget, system tray foundation.
- **Phase 2**: Kraken USB/HID handshake & descriptor verification.
- **Phase 3**: Kraken real-time telemetry packet parsing (Coolant temp, Pump RPM, Fan RPM).
- **Phase 4**: Physical LCD transport protocol for 240x240 / 320x320 / 640x640 displays.
- **Phase 5**: LCD custom image and animated GIF decoder & uploader.
- **Phase 6**: RGB lighting control (ring and logo LEDs).
- **Phase 7**: Pump and fan custom curve control.
- **Phase 8**: Hardware monitoring integration with native Windows PDH / NVML APIs.
- **Phase 9**: Custom profiles & presets with auto-switching based on active process.
