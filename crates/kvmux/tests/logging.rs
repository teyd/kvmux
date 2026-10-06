//! Run CLI-only commands to verify real startup/shutdown without touching hardware.

#[cfg(target_os = "linux")]
mod linux {
    use std::fs;
    use std::path::Path;
    use std::process::{Command, Output};

    fn run(state: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_kvmux"))
            .args(args)
            .env("XDG_STATE_HOME", state)
            .env_remove("RUST_LOG")
            .output()
            .expect("run kvmux")
    }

    #[test]
    fn version_has_no_logging_side_effects() {
        let state = tempfile::tempdir().expect("state directory");
        let output = run(state.path(), &["--version"]);
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("kvmux {}\n", env!("CARGO_PKG_VERSION"))
        );
        assert!(output.stderr.is_empty());
        assert!(!state.path().join("kvmux").exists());
    }

    #[test]
    fn each_launch_gets_a_log_and_only_five_are_retained() {
        let state = tempfile::tempdir().expect("state directory");
        for _ in 0..7 {
            let output = run(state.path(), &["set-input"]);
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
        }
        let files: Vec<_> = fs::read_dir(state.path().join("kvmux/logs"))
            .expect("logs directory")
            .collect();
        assert_eq!(files.len(), 5);
        for file in files {
            let log = fs::read_to_string(file.expect("log file").path()).expect("read log");
            assert!(log.contains("Starting kvmux"));
            assert!(log.contains("Missing set-input arguments"));
            assert!(log.contains("kvmux stopped"));
        }
    }

    #[test]
    fn file_logging_failure_preserves_command_behavior() {
        let state = tempfile::NamedTempFile::new().expect("unusable state directory");
        let output = run(state.path(), &["set-input"]);
        assert_eq!(output.status.code(), Some(2));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("File logging unavailable"));
        assert!(stderr.contains("usage:"));
    }
}
