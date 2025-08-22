//! RPC server configuration flags.

use clap::Args;
use std::net::IpAddr;

/// RPC server configuration for kona-node.
#[derive(Args, Debug, Clone)]
pub struct RpcArgs {
    /// Disable kona RPC server.
    #[arg(
        long = "kona.rpc.disable",
        default_value = "false",
        env = "KONA_RPC_DISABLED",
        help = "Disable the kona RPC server"
    )]
    pub rpc_disabled: bool,

    /// RPC server listening address.
    #[arg(
        long = "kona.rpc.addr",
        default_value = "0.0.0.0",
        env = "KONA_RPC_ADDR",
        help = "IP address for kona RPC server"
    )]
    pub addr: IpAddr,

    /// RPC server port.
    #[arg(
        long = "kona.rpc.port",
        default_value = "9546",
        env = "KONA_RPC_PORT",
        help = "Port for kona RPC server"
    )]
    pub port: u16,

    /// Enable admin API.
    #[arg(
        long = "kona.rpc.enable-admin",
        default_value = "false",
        env = "KONA_RPC_ENABLE_ADMIN",
        help = "Enable admin RPC methods"
    )]
    pub enable_admin: bool,

    /// Enable WebSocket support.
    #[arg(
        long = "kona.rpc.ws-enabled",
        default_value = "false",
        env = "KONA_RPC_WS_ENABLED",
        help = "Enable WebSocket RPC endpoint"
    )]
    pub ws_enabled: bool,

    /// Enable development RPC endpoints.
    #[arg(
        long = "kona.rpc.dev-enabled",
        default_value = "false",
        env = "KONA_RPC_DEV_ENABLED",
        help = "Enable development RPC endpoints for debugging"
    )]
    pub dev_enabled: bool,
}

impl Default for RpcArgs {
    fn default() -> Self {
        Self {
            rpc_disabled: false,
            addr: "0.0.0.0".parse().unwrap(),
            port: 9546,
            enable_admin: false,
            ws_enabled: false,
            dev_enabled: false,
        }
    }
}

impl RpcArgs {
    /// Validate RPC configuration.
    pub fn validate(&self) -> Result<(), String> {
        // Check for common port conflicts
        const RESERVED_PORTS: &[u16] = &[
            8545, // Common Ethereum RPC
            8546, // Common Ethereum WS
            8551, // Engine API
            9545, // Default kona-node RPC
        ];

        if !self.rpc_disabled && RESERVED_PORTS.contains(&self.port) {
            tracing::warn!("RPC port {} may conflict with other services", self.port);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_rpc_args() {
        let args = RpcArgs::default();
        assert!(!args.rpc_disabled);
        assert_eq!(args.port, 9546);
        assert!(!args.enable_admin);
        assert!(!args.ws_enabled);
        assert!(!args.dev_enabled);
    }

    #[test]
    fn test_parse_rpc_flags() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(flatten)]
            rpc: RpcArgs,
        }

        let args = vec![
            "test",
            "--kona.rpc.port",
            "7777",
            "--kona.rpc.enable-admin",
            "--kona.rpc.ws-enabled",
        ];
        let parsed = TestCli::try_parse_from(args).unwrap();
        assert_eq!(parsed.rpc.port, 7777);
        assert!(parsed.rpc.enable_admin);
        assert!(parsed.rpc.ws_enabled);
    }

    #[test]
    fn test_disabled_rpc() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(flatten)]
            rpc: RpcArgs,
        }

        let args = vec!["test", "--kona.rpc.disable"];
        let parsed = TestCli::try_parse_from(args).unwrap();
        assert!(parsed.rpc.rpc_disabled);
    }

    #[test]
    fn test_validate_rpc() {
        let args = RpcArgs::default();
        assert!(args.validate().is_ok());

        // Even reserved ports should validate OK (just warn)
        let mut args = RpcArgs::default();
        args.port = 8545;
        assert!(args.validate().is_ok());
    }
}
