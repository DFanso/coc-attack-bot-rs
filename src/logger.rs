use std::path::PathBuf;
use time::macros::format_description;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::time::LocalTime;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init() -> anyhow::Result<WorkerGuard> {
    let log_dir = PathBuf::from("logs");
    std::fs::create_dir_all(&log_dir)?;

    let today = time::OffsetDateTime::now_local()
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let stamp = today.format(format_description!("[year][month][day]")).unwrap();
    let log_file = log_dir.join(format!("coc_bot_{stamp}.log"));

    let appender = tracing_appender::rolling::never(
        log_file.parent().unwrap(),
        log_file.file_name().unwrap(),
    );
    let (non_blocking, guard) = tracing_appender::non_blocking(appender);

    let timer = LocalTime::new(format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second]"
    ));

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_timer(timer.clone())
        .with_target(false);

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_timer(timer)
        .with_target(false);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .with(stdout_layer)
        .init();

    tracing::info!("Logger initialized → {}", log_file.display());
    Ok(guard)
}
