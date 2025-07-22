//! Arguments for logging.

use std::path::PathBuf;

use clap::{ArgAction, Args, ValueEnum};
use serde::{Deserialize, Serialize};
use tracing::level_filters::LevelFilter;

use crate::tracing::LogFormat;

/// Global configuration arguments.
#[derive(Args, Debug, Default, Serialize, Deserialize, Clone)]
pub struct LogArgs {
    /// Verbosity level (1-5).
    /// By default, the verbosity level is set to 3 (info level).
    ///
    /// This verbosity level is shared by both stdout and file logging (if enabled).
    #[arg(
        short = 'v',
        global = true,
        default_value = "3",
        env = "KONA_NODE_LOG_LEVEL",
        action = ArgAction::Count,
    )]
    pub level: u8,
    /// If set, no logs are printed to stdout.
    #[arg(
        long = "logs.stdout.quiet",
        short = 'q',
        global = true,
        default_value = "false",
        env = "KONA_NODE_STDOUT_LOG_QUIET"
    )]
    pub stdout_quiet: bool,
    /// The format of the logs printed to stdout. One of: full, json, pretty, compact.
    ///
    /// full: The default rust log format.
    /// json: The logs are printed in JSON structured format.
    /// pretty: The logs are printed in a pretty, human readable format.
    /// compact: The logs are printed in a compact format.
    #[arg(
        long = "logs.stdout.format",
        default_value = "full",
        env = "KONA_NODE_LOG_STDOUT_FORMAT"
    )]
    pub stdout_format: LogFormat,
    /// The directory to store the log files.
    /// If not set, no logs are printed to files.
    #[arg(long = "logs.file.directory", env = "KONA_NODE_LOG_FILE_DIRECTORY")]
    pub file_directory: Option<PathBuf>,
    /// The format of the logs printed to log files. One of: full, json, pretty, compact.
    ///
    /// full: The default rust log format.
    /// json: The logs are printed in JSON structured format.
    /// pretty: The logs are printed in a pretty, human readable format.
    /// compact: The logs are printed in a compact format.
    #[arg(long = "logs.file.format", default_value = "full", env = "KONA_NODE_LOG_FILE_FORMAT")]
    pub file_format: LogFormat,
    /// The rotation of the log files. One of: hourly, daily, weekly, monthly, never.
    /// If set, new log files will be created every interval.
    #[arg(
        long = "logs.file.rotation",
        default_value = "never",
        env = "KONA_NODE_LOG_FILE_ROTATION"
    )]
    pub file_rotation: LogRotation,
    /// Maximum number of log files to keep. Older files will be deleted when this limit is exceeded.
    /// If not set, log files will not be automatically cleaned up.
    #[arg(
        long = "logs.file.max-files",
        env = "KONA_NODE_LOG_FILE_MAX_FILES"
    )]
    pub file_max_files: Option<usize>,
    /// Maximum age of log files in days. Older files will be deleted.
    /// If not set, log files will not be automatically cleaned up based on age.
    #[arg(
        long = "logs.file.max-age-days",
        env = "KONA_NODE_LOG_FILE_MAX_AGE_DAYS"
    )]
    pub file_max_age_days: Option<u64>,
}

/// The rotation of the log files.
#[derive(Debug, Clone, Serialize, Deserialize, ValueEnum, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogRotation {
    /// Rotate the log files every minute.
    Minutely,
    /// Rotate the log files hourly.
    Hourly,
    /// Rotate the log files daily.
    Daily,
    /// Do not rotate the log files.
    #[default]
    Never,
}

/// Configuration for file logging.
#[derive(Debug, Clone)]
pub struct FileLogConfig {
    /// The path to the directory where the log files are stored.
    pub directory_path: PathBuf,
    /// The format of the logs printed to the log file.
    pub format: LogFormat,
    /// The rotation of the log files.
    pub rotation: LogRotation,
    /// Maximum number of log files to keep.
    pub max_files: Option<usize>,
    /// Maximum age of log files in days.
    pub max_age_days: Option<u64>,
}

/// Configuration for stdout logging.
#[derive(Debug, Clone)]
pub struct StdoutLogConfig {
    /// The format of the logs printed to stdout.
    pub format: LogFormat,
}

/// Global configuration for logging.
/// Default is to only print logs to stdout in full format.
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Global verbosity level for logging.
    pub global_level: LevelFilter,
    /// The configuration for stdout logging.
    pub stdout_logs: Option<StdoutLogConfig>,
    /// The configuration for file logging.
    pub file_logs: Option<FileLogConfig>,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            global_level: LevelFilter::INFO,
            stdout_logs: Some(StdoutLogConfig { format: LogFormat::Full }),
            file_logs: None,
        }
    }
}

impl From<LogArgs> for LogConfig {
    fn from(args: LogArgs) -> Self {
        Self::new(args)
    }
}

impl LogConfig {
    /// Creates a new `LogConfig` from `LogArgs`.
    pub fn new(args: LogArgs) -> Self {
        let level = match args.level {
            1 => LevelFilter::ERROR,
            2 => LevelFilter::WARN,
            3 => LevelFilter::INFO,
            4 => LevelFilter::DEBUG,
            _ => LevelFilter::TRACE,
        };

        let stdout_logs = if args.stdout_quiet {
            None
        } else {
            Some(StdoutLogConfig { format: args.stdout_format })
        };

        let file_logs = args.file_directory.as_ref().map(|path| FileLogConfig {
            directory_path: path.clone(),
            format: args.file_format,
            rotation: args.file_rotation,
            max_files: args.file_max_files,
            max_age_days: args.file_max_age_days,
        });

        Self { global_level: level, stdout_logs, file_logs }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    // Helper struct to parse GlobalArgs within a test CLI structure
    #[derive(Parser, Debug)]
    struct TestCli {
        #[command(flatten)]
        global: LogArgs,
    }

    #[test]
    fn test_default_verbosity_level() {
        let cli = TestCli::parse_from(["test_app"]);
        assert_eq!(
            cli.global.level, 3,
            "Default verbosity should be 3 when no -v flag is present."
        );
    }

    #[test]
    fn test_verbosity_count() {
        let cli_v1 = TestCli::parse_from(["test_app", "-v"]);
        assert_eq!(cli_v1.global.level, 1, "Verbosity with a single -v should be 1.");

        let cli_v3 = TestCli::parse_from(["test_app", "-vvv"]);
        assert_eq!(cli_v3.global.level, 3, "Verbosity with -vvv should be 3.");

        let cli_v5 = TestCli::parse_from(["test_app", "-vvvvv"]);
        assert_eq!(cli_v5.global.level, 5, "Verbosity with -vvvvv should be 5.");
    }

    #[test]
    fn test_log_cleanup_args_parsing() {
        let cli = TestCli::parse_from([
            "test_app",
            "--logs.file.max-files", "10",
            "--logs.file.max-age-days", "7"
        ]);
        assert_eq!(cli.global.file_max_files, Some(10));
        assert_eq!(cli.global.file_max_age_days, Some(7));
    }

    #[test]
    fn test_log_config_includes_cleanup_options() {
        use std::path::PathBuf;

        let args = LogArgs {
            level: 3,
            stdout_quiet: false,
            stdout_format: crate::tracing::LogFormat::Full,
            file_directory: Some(PathBuf::from("/tmp/test")),
            file_format: crate::tracing::LogFormat::Json,
            file_rotation: LogRotation::Daily,
            file_max_files: Some(5),
            file_max_age_days: Some(14),
        };

        let config = LogConfig::new(args);
        
        assert!(config.file_logs.is_some());
        let file_config = config.file_logs.unwrap();
        assert_eq!(file_config.max_files, Some(5));
        assert_eq!(file_config.max_age_days, Some(14));
    }
}
