use std::{
    backtrace::Backtrace,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use tauri::{AppHandle, Runtime};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::error::{AppError, Result};

const LOG_RETENTION: Duration = Duration::from_secs(604_800);

pub struct LoggingGuard {
    _guard: tracing_appender::non_blocking::WorkerGuard,
}

pub fn init(log_dir: &Path) -> Result<LoggingGuard> {
    let cutoff = SystemTime::now()
        .checked_sub(LOG_RETENTION)
        .ok_or_else(|| AppError::Internal("could not calculate log retention cutoff".to_owned()))?;
    prune_logs_before(log_dir, cutoff)?;
    let appender = tracing_appender::rolling::daily(log_dir, "trident.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(writer))
        .try_init()
        .map_err(|error| AppError::Internal(error.to_string()))?;

    Ok(LoggingGuard { _guard: guard })
}

pub fn install_panic_hook<R: Runtime + 'static>(app: AppHandle<R>, log_dir: PathBuf) {
    std::panic::set_hook(Box::new(move |_| {
        let backtrace = Backtrace::force_capture();
        tracing::error!(backtrace = %backtrace, "Trident panic captured");
        app.dialog()
            .message(format!(
                "Trident stopped unexpectedly. Logs: {}",
                log_dir.display()
            ))
            .kind(MessageDialogKind::Error)
            .title("Trident error")
            .show(|_| {});
    }));
}

fn prune_logs_before(log_dir: &Path, cutoff: SystemTime) -> Result<()> {
    for entry in fs::read_dir(log_dir)? {
        let entry = entry?;
        let path = entry.path();
        let is_trident_log = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("trident."));

        if is_trident_log && entry.metadata()?.modified()? < cutoff {
            fs::remove_file(path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use tempfile::TempDir;

    use super::{prune_logs_before, LOG_RETENTION};

    #[test]
    fn removes_expired_trident_logs_and_preserves_other_files() -> crate::error::Result<()> {
        let directory = TempDir::new()?;
        let trident_log = directory.path().join("trident.log");
        let fallback_log = directory.path().join("capture-fallback.log");
        fs::write(&trident_log, [])?;
        fs::write(&fallback_log, [])?;
        let cutoff = SystemTime::now()
            .checked_add(LOG_RETENTION)
            .ok_or_else(|| {
                crate::error::AppError::Internal("could not calculate test cutoff".to_owned())
            })?;

        prune_logs_before(directory.path(), cutoff)?;

        assert!(!trident_log.exists());
        assert!(fallback_log.exists());
        Ok(())
    }

    #[test]
    fn reports_an_io_error_when_log_path_is_not_a_directory() -> crate::error::Result<()> {
        let directory = TempDir::new()?;
        let log_path = directory.path().join("trident.log");
        fs::write(&log_path, [])?;

        let result = prune_logs_before(&log_path, SystemTime::now());

        assert!(matches!(result, Err(crate::error::AppError::Io(_))));
        Ok(())
    }
}
