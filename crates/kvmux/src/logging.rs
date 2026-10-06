//! Per-launch logging. Keep the returned guard alive until the application exits.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use directories::BaseDirs;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt as _, util::SubscriberInitExt as _};

const RETAIN_FILES: usize = 5;

/// Installs timestamped, structured text logs on stderr and in a per-launch file.
/// File errors fall back to stderr rather than preventing the application from starting.
/// `RUST_LOG` overrides the default application-info/dependency-warning filter.
pub fn init() -> Option<WorkerGuard> {
    let file = log_directory().and_then(|directory| create_session(&directory));
    let (writer, guard, file_error) = match file {
        Ok((file, path)) => {
            let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
                .lossy(false)
                .finish(file);
            (Some(writer), Some(guard), Ok(path))
        }
        Err(error) => (None, None, Err(error)),
    };
    let filter = EnvFilter::try_from_default_env();
    let filter_error = filter.as_ref().err().map(ToString::to_string);
    let filter = filter.unwrap_or_else(|_| EnvFilter::new("warn,kvmux=info,kvmux_core=info"));
    let file_layer = writer.map(|writer| {
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(writer)
    });
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(io::stderr),
        )
        .with(file_layer)
        .init();
    if std::env::var_os("RUST_LOG").is_some()
        && let Some(error) = filter_error
    {
        tracing::warn!(%error, "Invalid RUST_LOG; using the default filter");
    }
    match file_error {
        Ok(path) => tracing::info!(log_file = %path.display(), "Logging initialized"),
        Err(error) => tracing::warn!(%error, "File logging unavailable; using stderr only"),
    }
    guard
}

fn log_directory() -> io::Result<PathBuf> {
    let dirs = BaseDirs::new().ok_or_else(|| io::Error::other("No user data directory"))?;
    Ok(dirs
        .state_dir()
        .unwrap_or(dirs.data_local_dir())
        .join("kvmux")
        .join("logs"))
}

fn create_session(directory: &Path) -> io::Result<(File, PathBuf)> {
    fs::create_dir_all(directory)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let (file, path) = create_file(directory, timestamp)?;
    // Retention failure must not disable an otherwise usable log file.
    if let Err(error) = prune(directory, &path, RETAIN_FILES) {
        use std::io::Write as _;
        writeln!(&file, "Could not prune old log files: {error}")?;
    }
    Ok((file, path))
}

fn create_file(directory: &Path, timestamp: u128) -> io::Result<(File, PathBuf)> {
    for sequence in 0..u32::MAX {
        let path = directory.join(format!(
            "kvmux-{timestamp:039}-{:010}-{sequence:010}.log",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other("Could not allocate a unique log filename"))
}

fn is_session_name(name: &str) -> bool {
    let Some(stem) = name
        .strip_prefix("kvmux-")
        .and_then(|s| s.strip_suffix(".log"))
    else {
        return false;
    };
    let parts: Vec<_> = stem.split('-').collect();
    parts.len() == 3
        && parts
            .iter()
            .zip([39, 10, 10])
            .all(|(part, len)| part.len() == len && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn prune(directory: &Path, current: &Path, retain: usize) -> io::Result<()> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.file_name().to_str().is_some_and(is_session_name)
            && entry.path() != current
        {
            files.push(entry.path());
        }
    }
    // Fixed-width timestamps sort chronologically; always protect this launch's file.
    files.sort();
    let remove = files.len().saturating_sub(retain.saturating_sub(1));
    for path in files.into_iter().take(remove) {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn retains_five_newest_sessions_and_preserves_unrelated_files() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let unrelated = dir.path().join("kvmux-not-a-session.log");
        fs::write(&unrelated, "keep").expect("unrelated file");
        let mut paths = Vec::new();
        for timestamp in 0..8 {
            let (file, path) = create_file(dir.path(), timestamp).expect("session");
            drop(file);
            prune(dir.path(), &path, RETAIN_FILES).expect("retention");
            paths.push(path);
        }
        assert!(paths[..3].iter().all(|path| !path.exists()));
        assert!(paths[3..].iter().all(|path| path.exists()));
        assert_eq!(fs::read_to_string(unrelated).expect("preserved"), "keep");
    }

    #[test]
    fn identical_timestamps_never_overwrite_existing_logs() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let (mut first, first_path) = create_file(dir.path(), 42).expect("first");
        writeln!(first, "original").expect("write");
        let (_, second_path) = create_file(dir.path(), 42).expect("second");
        assert_ne!(first_path, second_path);
        assert_eq!(
            fs::read_to_string(first_path).expect("original"),
            "original\n"
        );
    }

    #[test]
    fn protects_current_log_when_clock_moves_backwards() {
        let dir = tempfile::tempdir().expect("temporary directory");
        for timestamp in 10..15 {
            create_file(dir.path(), timestamp).expect("old session");
        }
        let (_, current) = create_file(dir.path(), 1).expect("new session");
        prune(dir.path(), &current, RETAIN_FILES).expect("retention");
        assert!(current.exists());
        assert_eq!(fs::read_dir(dir.path()).expect("logs").count(), 5);
    }

    #[test]
    fn session_creates_directory_and_flushes_on_shutdown() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let (file, path) = create_session(&dir.path().join("logs")).expect("session");
        let (writer, guard) = tracing_appender::non_blocking(file);
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter("info")
            .with_ansi(false)
            .with_writer(writer)
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!("filtered event");
            tracing::info!(monitor_id = "DEL:123:ABC", "last event");
        });
        drop(guard);
        let log = fs::read_to_string(path).expect("log");
        assert!(log.contains("INFO"));
        assert!(log.contains("last event"));
        assert!(log.contains("monitor_id=\"DEL:123:ABC\""));
        assert!(!log.contains("filtered event"));
        assert!(!log.contains('\u{1b}'));
    }

    #[test]
    fn unusable_directory_returns_an_error() {
        let file = tempfile::NamedTempFile::new().expect("temporary file");
        assert!(create_session(file.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn retention_ignores_symlinks() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let target = dir.path().join("unrelated.txt");
        fs::write(&target, "keep").expect("target");
        let (_, path) = create_file(dir.path(), 0).expect("filename");
        fs::remove_file(&path).expect("remove file");
        std::os::unix::fs::symlink(&target, &path).expect("symlink");
        let (_, current) = create_file(dir.path(), 1).expect("session");
        prune(dir.path(), &current, 1).expect("retention");
        assert!(path.is_symlink());
        assert_eq!(fs::read_to_string(target).expect("preserved"), "keep");
    }
}
