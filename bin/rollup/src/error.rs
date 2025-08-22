//! Error types for the rollup binary.

use thiserror::Error;

/// Main error type for the rollup binary CLI operations.
#[derive(Error, Debug)]
pub enum RollupError {
    /// Configuration validation error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// CLI parsing error.
    #[error("CLI error: {0}")]
    Cli(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// URL parsing error.
    #[error("URL parsing error: {0}")]
    UrlParse(#[from] url::ParseError),

    /// Address parsing error.
    #[error("Address parsing error: {0}")]
    AddrParse(#[from] std::net::AddrParseError),

    /// Other error.
    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

/// Result type alias for rollup operations.
pub type RollupResult<T> = Result<T, RollupError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error() {
        let error = RollupError::Config("test config error".to_string());
        assert_eq!(error.to_string(), "Configuration error: test config error");
    }

    #[test]
    fn test_cli_error() {
        let error = RollupError::Cli("test cli error".to_string());
        assert_eq!(error.to_string(), "CLI error: test cli error");
    }
}