pub mod app;
pub mod config;
pub mod hardware;
pub mod monitoring;
pub mod nzxt;
pub mod rendering;
pub mod windows;

use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::app::Application;
use crate::config::AppConfig;
use crate::windows::{MainWindow, MessageLoop};

fn main() {
    // 1. Initialize structured logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_ansi(true))
        .init();

    info!("=================================================");
    info!("   AIPulse - Native Windows AIO Controller       ");
    info!("   Target: NZXT Kraken 360                       ");
    info!("=================================================");

    // 2. Load or create versioned configuration
    let config = AppConfig::load_or_default();
    info!("Application configuration initialized (v{}).", config.version);

    // 3. Initialize application coordinator
    let app = Arc::new(Application::new(config));

    // 4. Create main native Win32 window and start subsystems
    if let Err(err) = MainWindow::create_and_run(Arc::clone(&app)) {
        error!("Failed to create main application window: {}", err);
        return;
    }

    // 5. Run native Win32 event-driven message loop
    info!("Entering Win32 message loop (event-driven, 0% idle CPU)...");
    let exit_code = MessageLoop::run();
    info!("Application exited cleanly with code {}.", exit_code);
}
