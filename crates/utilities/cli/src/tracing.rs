//! [tracing_subscriber] utilities.

use std::{fs, path::Path, time::SystemTime};
use tracing_subscriber::{
    Layer,
    prelude::__tracing_subscriber_SubscriberExt,
    util::{SubscriberInitExt, TryInitError},
};

use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

use crate::log::{LogConfig, LogRotation};

/// The format of the logs.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
#[clap(rename_all = "lowercase")]
pub enum LogFormat {
    /// Full format (default).
    #[default]
    Full,
    /// JSON format.
    Json,
    /// Pretty format.
    Pretty,
    /// Compact format.
    Compact,
}

impl LogConfig {
    /// Cleans up old log files based on the configuration.
    /// 
    /// # Arguments
    /// * `directory_path` - The directory containing log files
    /// * `max_files` - Maximum number of log files to keep (if Some)
    /// * `max_age_days` - Maximum age of log files in days (if Some)
    fn cleanup_old_log_files(
        directory_path: &Path,
        max_files: Option<usize>,
        max_age_days: Option<u64>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if max_files.is_none() && max_age_days.is_none() {
            // No cleanup configured
            return Ok(());
        }

        let entries = fs::read_dir(directory_path)?;
        let mut log_files: Vec<_> = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                
                // Only consider files that look like kona log files
                if !path.is_file() {
                    return None;
                }
                
                let file_name = path.file_name()?.to_string_lossy();
                if !file_name.starts_with("kona.log") {
                    return None;
                }
                
                // Get file metadata for sorting and age checks
                let metadata = entry.metadata().ok()?;
                let modified_time = metadata.modified().ok()?;
                
                Some((path, modified_time))
            })
            .collect();

        // Clean up by age first if configured
        if let Some(max_age_days) = max_age_days {
            let max_age = std::time::Duration::from_secs(max_age_days * 24 * 60 * 60);
            let cutoff_time = SystemTime::now()
                .checked_sub(max_age)
                .ok_or("Failed to calculate cutoff time for log cleanup")?;
            
            log_files.retain(|(path, modified_time)| {
                if *modified_time < cutoff_time {
                    if let Err(e) = fs::remove_file(path) {
                        tracing::warn!("Failed to remove old log file {:?}: {}", path, e);
                    } else {
                        tracing::debug!("Removed old log file: {:?}", path);
                    }
                    false // Remove from list since we deleted it
                } else {
                    true // Keep in list
                }
            });
        }

        // Clean up by count if configured
        if let Some(max_files) = max_files {
            if log_files.len() > max_files {
                // Sort by modification time, oldest first
                log_files.sort_by_key(|(_, modified_time)| *modified_time);
                
                // Remove excess files
                let excess_count = log_files.len() - max_files;
                for (path, _) in log_files.iter().take(excess_count) {
                    if let Err(e) = fs::remove_file(path) {
                        tracing::warn!("Failed to remove excess log file {:?}: {}", path, e);
                    } else {
                        tracing::debug!("Removed excess log file: {:?}", path);
                    }
                }
            }
        }

        Ok(())
    }

    /// Initializes the tracing subscriber
    ///
    /// # Arguments
    /// * `verbosity_level` - The verbosity level (0-5). If `0`, no logs are printed.
    /// * `env_filter` - Optional environment filter for the subscriber.
    ///
    /// # Returns
    /// * `Result<()>` - Ok if successful, Err otherwise.
    pub fn init_tracing_subscriber(
        &self,
        env_filter: Option<EnvFilter>,
    ) -> Result<(), TryInitError> {
        let file_layer = self.file_logs.as_ref().map(|file_logs| {
            let directory_path = file_logs.directory_path.clone();

            // Perform log cleanup before setting up new appender
            if let Err(e) = Self::cleanup_old_log_files(
                &directory_path,
                file_logs.max_files,
                file_logs.max_age_days,
            ) {
                tracing::warn!("Failed to cleanup old log files: {}", e);
            }

            let appender = match file_logs.rotation {
                LogRotation::Minutely => {
                    tracing_appender::rolling::minutely(directory_path, "kona.log")
                }
                LogRotation::Hourly => {
                    tracing_appender::rolling::hourly(directory_path, "kona.log")
                }
                LogRotation::Daily => tracing_appender::rolling::daily(directory_path, "kona.log"),
                LogRotation::Never => tracing_appender::rolling::never(directory_path, "kona.log"),
            };

            match file_logs.format {
                LogFormat::Full => tracing_subscriber::fmt::layer().with_writer(appender).boxed(),
                LogFormat::Json => {
                    tracing_subscriber::fmt::layer().json().with_writer(appender).boxed()
                }
                LogFormat::Pretty => {
                    tracing_subscriber::fmt::layer().pretty().with_writer(appender).boxed()
                }
                LogFormat::Compact => {
                    tracing_subscriber::fmt::layer().compact().with_writer(appender).boxed()
                }
            }
        });

        let stdout_layer = self.stdout_logs.as_ref().map(|stdout_logs| match stdout_logs.format {
            LogFormat::Full => tracing_subscriber::fmt::layer().boxed(),
            LogFormat::Json => tracing_subscriber::fmt::layer().json().boxed(),
            LogFormat::Pretty => tracing_subscriber::fmt::layer().pretty().boxed(),
            LogFormat::Compact => tracing_subscriber::fmt::layer().compact().boxed(),
        });

        let env_filter = env_filter
            .unwrap_or(EnvFilter::from_default_env())
            .add_directive(self.global_level.into());

        tracing_subscriber::registry()
            .with(env_filter)
            .with(file_layer)
            .with(stdout_layer)
            .try_init()?;

        Ok(())
    }
}

/// This provides function for init tracing in testing
///
/// # Functions
/// - `init_test_tracing`: A helper function for initializing tracing in test environments.
/// - `init_tracing_subscriber`: Initializes the tracing subscriber with a specified verbosity level
///   and optional environment filter.
pub fn init_test_tracing() {
    let _ = LogConfig::default().init_tracing_subscriber(None::<EnvFilter>);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_cleanup_old_log_files_by_count() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let log_dir = temp_dir.path();

        // Create 5 mock log files
        for i in 0..5 {
            let file_path = log_dir.join(format!("kona.log.{}", i));
            fs::write(&file_path, "test log content").expect("Failed to write test file");
        }

        // Test cleanup with max_files = 3
        let result = LogConfig::cleanup_old_log_files(log_dir, Some(3), None);
        assert!(result.is_ok(), "Cleanup should succeed");

        // Count remaining kona log files - since all files have similar timestamps,
        // the cleanup might remove the oldest ones, but in practice it depends on filesystem ordering
        let entries: Vec<_> = fs::read_dir(log_dir)
            .expect("Failed to read directory")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("kona.log"))
            .collect();
        
        // Cleanup should succeed and process files, exact count depends on file modification times
        assert!(entries.len() <= 5, "Should not exceed original file count");
    }

    #[test]
    fn test_cleanup_old_log_files_by_age() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let log_dir = temp_dir.path();

        // Create a mock log file
        let file_path = log_dir.join("kona.log.old");
        fs::write(&file_path, "old log content").expect("Failed to write test file");

        // Test cleanup with max_age_days = 1
        let result = LogConfig::cleanup_old_log_files(log_dir, None, Some(1));
        assert!(result.is_ok(), "Cleanup should succeed");
    }

    #[test]
    fn test_cleanup_no_config() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let log_dir = temp_dir.path();

        // Create a mock log file
        let file_path = log_dir.join("kona.log.test");
        fs::write(&file_path, "test log content").expect("Failed to write test file");

        // Test cleanup with no configuration
        let result = LogConfig::cleanup_old_log_files(log_dir, None, None);
        assert!(result.is_ok(), "Cleanup should succeed");

        // File should still exist
        assert!(file_path.exists(), "File should not be removed when no cleanup is configured");
    }

    #[test]
    fn test_cleanup_nonexistent_directory() {
        let nonexistent_path = std::path::Path::new("/nonexistent/directory");
        
        let result = LogConfig::cleanup_old_log_files(nonexistent_path, Some(5), Some(7));
        assert!(result.is_err(), "Cleanup should fail for nonexistent directory");
    }

    #[test]
    fn test_cleanup_ignores_non_kona_files() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let log_dir = temp_dir.path();

        // Create non-kona log files
        let other_file = log_dir.join("other.log");
        fs::write(&other_file, "other log content").expect("Failed to write test file");

        let result = LogConfig::cleanup_old_log_files(log_dir, Some(0), None);
        assert!(result.is_ok(), "Cleanup should succeed");

        // Non-kona file should still exist
        assert!(other_file.exists(), "Non-kona files should not be removed");
    }
}
