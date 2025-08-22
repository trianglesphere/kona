//! Sequencer configuration flags.

use clap::Args;
use url::Url;

/// Sequencer configuration.
#[derive(Args, Debug, Clone)]
pub struct SequencerArgs {
    /// Start sequencer in stopped state.
    #[arg(
        long = "sequencer.stopped",
        default_value = "false",
        env = "SEQUENCER_STOPPED",
        help = "Start the sequencer in a stopped state"
    )]
    pub stopped: bool,

    /// Maximum safe lag in L2 blocks.
    #[arg(
        long = "sequencer.max-safe-lag",
        default_value = "0",
        env = "SEQUENCER_MAX_SAFE_LAG",
        help = "Maximum number of L2 blocks between safe and unsafe (0 = disabled)"
    )]
    pub max_safe_lag: u64,

    /// L1 confirmations for sequencer.
    #[arg(
        long = "sequencer.l1-confs",
        default_value = "4",
        env = "SEQUENCER_L1_CONFS",
        help = "Number of L1 confirmations for sequencer to wait"
    )]
    pub l1_confs: u64,

    /// Enable recover mode.
    #[arg(
        long = "sequencer.recover",
        default_value = "false",
        env = "SEQUENCER_RECOVER",
        help = "Force sequencer to prepare next L1 origin and create empty L2 blocks"
    )]
    pub recover: bool,

    /// L2 sequencer HTTP endpoint.
    #[arg(
        long = "sequencer.l2",
        env = "SEQUENCER_L2",
        help = "HTTP endpoint for the L2 sequencer"
    )]
    pub sequencer_l2: Option<Url>,

    /// Conductor RPC endpoint.
    #[arg(
        long = "conductor.rpc",
        env = "CONDUCTOR_RPC",
        help = "Conductor service RPC endpoint (enables conductor mode)"
    )]
    pub conductor_rpc: Option<Url>,

    /// Conductor RPC timeout in seconds.
    #[arg(
        long = "conductor.rpc-timeout",
        default_value = "30",
        env = "CONDUCTOR_RPC_TIMEOUT",
        help = "Timeout for conductor RPC calls in seconds"
    )]
    pub conductor_rpc_timeout: u64,
}

impl Default for SequencerArgs {
    fn default() -> Self {
        Self {
            stopped: false,
            max_safe_lag: 0,
            l1_confs: 4,
            recover: false,
            sequencer_l2: None,
            conductor_rpc: None,
            conductor_rpc_timeout: 30,
        }
    }
}

impl SequencerArgs {
    /// Check if conductor mode is enabled.
    pub fn is_conductor_enabled(&self) -> bool {
        self.conductor_rpc.is_some()
    }

    /// Validate sequencer configuration.
    pub fn validate(&self) -> Result<(), String> {
        if self.l1_confs == 0 {
            return Err("Sequencer L1 confirmations must be greater than 0".to_string());
        }

        if self.conductor_rpc_timeout == 0 {
            return Err("Conductor RPC timeout must be greater than 0".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_sequencer_args() {
        let args = SequencerArgs::default();
        assert!(!args.stopped);
        assert_eq!(args.max_safe_lag, 0);
        assert_eq!(args.l1_confs, 4);
        assert!(!args.recover);
        assert!(args.conductor_rpc.is_none());
        assert_eq!(args.conductor_rpc_timeout, 30);
    }

    #[test]
    fn test_conductor_enabled() {
        let mut args = SequencerArgs::default();
        assert!(!args.is_conductor_enabled());

        args.conductor_rpc = Some("http://conductor.example.com".parse().unwrap());
        assert!(args.is_conductor_enabled());
    }

    #[test]
    fn test_validate_l1_confs() {
        let mut args = SequencerArgs::default();
        args.l1_confs = 0;
        assert!(args.validate().is_err());

        args.l1_confs = 1;
        assert!(args.validate().is_ok());
    }

    #[test]
    fn test_validate_timeout() {
        let mut args = SequencerArgs::default();
        args.conductor_rpc_timeout = 0;
        assert!(args.validate().is_err());

        args.conductor_rpc_timeout = 1;
        assert!(args.validate().is_ok());
    }

    #[test]
    fn test_parse_sequencer_flags() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(flatten)]
            seq: SequencerArgs,
        }

        let args = vec![
            "test",
            "--sequencer.stopped",
            "--sequencer.max-safe-lag",
            "100",
            "--sequencer.l1-confs",
            "10",
            "--sequencer.recover",
            "--conductor.rpc",
            "http://conductor.local",
        ];
        let parsed = TestCli::try_parse_from(args).unwrap();
        assert!(parsed.seq.stopped);
        assert_eq!(parsed.seq.max_safe_lag, 100);
        assert_eq!(parsed.seq.l1_confs, 10);
        assert!(parsed.seq.recover);
        assert_eq!(parsed.seq.conductor_rpc.unwrap().as_str(), "http://conductor.local/");
    }
}
