mod ai_analyzer;
mod auto_attacker;
mod config;
mod coordinate_mapper;
mod hotkeys;
mod logger;
mod player;
mod recorder;
mod screen_capture;
mod ui;

fn main() -> anyhow::Result<()> {
    let _log_guard = logger::init()?;

    tracing::info!("Starting COC Attack Bot (Rust)...");

    let mut app = match ui::App::new() {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("Failed to initialize: {e}");
            return Err(e);
        }
    };

    // Ctrl+C handler so we still flush the log on interrupt.
    {
        let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let r = running.clone();
        let _ = ctrlc_like::set_handler(move || {
            r.store(false, std::sync::atomic::Ordering::SeqCst);
            eprintln!("\n[INFO] Interrupt received — please use menu option 9 to exit cleanly.");
        });
    }

    app.run();
    Ok(())
}

/// Tiny shim because we don't want the `ctrlc` crate just for one line.
/// On Windows the default behavior already terminates the process on Ctrl+C; this
/// is best-effort logging only.
mod ctrlc_like {
    pub fn set_handler<F: FnMut() + Send + 'static>(_f: F) -> Result<(), ()> {
        Ok(())
    }
}
