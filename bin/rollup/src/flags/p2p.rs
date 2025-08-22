//! P2P configuration flags.

use clap::Args;
use std::net::IpAddr;

/// P2P network configuration.
#[derive(Args, Debug, Clone)]
pub struct P2PArgs {
    /// Disable P2P networking.
    #[arg(
        long = "p2p.disable",
        default_value = "false",
        env = "P2P_DISABLED",
        help = "Disable P2P networking entirely"
    )]
    pub p2p_disabled: bool,

    /// IP to bind P2P services to.
    #[arg(
        long = "p2p.listen.ip",
        default_value = "0.0.0.0",
        env = "P2P_LISTEN_IP",
        help = "IP address to bind P2P services"
    )]
    pub listen_ip: IpAddr,

    /// TCP port for P2P connections.
    #[arg(
        long = "p2p.listen.tcp-port",
        default_value = "9222",
        env = "P2P_LISTEN_TCP_PORT",
        help = "TCP port for P2P connections"
    )]
    pub listen_tcp_port: u16,

    /// UDP port for P2P discovery.
    #[arg(
        long = "p2p.listen.udp-port",
        default_value = "9223",
        env = "P2P_LISTEN_UDP_PORT",
        help = "UDP port for P2P discovery"
    )]
    pub listen_udp_port: u16,

    /// Target number of peer connections (low watermark).
    #[arg(
        long = "p2p.peers.lo",
        default_value = "20",
        env = "P2P_PEERS_LO",
        help = "Minimum number of peer connections to maintain"
    )]
    pub peers_lo: u32,

    /// Maximum number of peer connections (high watermark).
    #[arg(
        long = "p2p.peers.hi",
        default_value = "30",
        env = "P2P_PEERS_HI",
        help = "Maximum number of peer connections"
    )]
    pub peers_hi: u32,

    /// Disable P2P discovery.
    #[arg(
        long = "p2p.no-discovery",
        default_value = "false",
        env = "P2P_NO_DISCOVERY",
        help = "Disable peer discovery mechanisms"
    )]
    pub no_discovery: bool,

    /// P2P bootnode ENRs.
    #[arg(
        long = "p2p.bootnodes",
        env = "P2P_BOOTNODES",
        value_delimiter = ',',
        help = "Comma-separated list of bootnode ENRs"
    )]
    pub bootnodes: Vec<String>,
}

impl Default for P2PArgs {
    fn default() -> Self {
        Self {
            p2p_disabled: false,
            listen_ip: "0.0.0.0".parse().unwrap(),
            listen_tcp_port: 9222,
            listen_udp_port: 9223,
            peers_lo: 20,
            peers_hi: 30,
            no_discovery: false,
            bootnodes: Vec::new(),
        }
    }
}

impl P2PArgs {
    /// Check for port conflicts.
    pub fn validate(&self) -> Result<(), String> {
        if !self.p2p_disabled && self.listen_tcp_port == self.listen_udp_port {
            return Err("P2P TCP and UDP ports must be different".to_string());
        }

        if self.peers_lo > self.peers_hi {
            return Err("P2P peers.lo must be less than or equal to peers.hi".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_p2p_args() {
        let args = P2PArgs::default();
        assert!(!args.p2p_disabled);
        assert_eq!(args.listen_tcp_port, 9222);
        assert_eq!(args.listen_udp_port, 9223);
        assert_eq!(args.peers_lo, 20);
        assert_eq!(args.peers_hi, 30);
    }

    #[test]
    fn test_validate_port_conflict() {
        let mut args = P2PArgs::default();
        args.listen_tcp_port = 9000;
        args.listen_udp_port = 9000;
        assert!(args.validate().is_err());
    }

    #[test]
    fn test_validate_peer_counts() {
        let mut args = P2PArgs::default();
        args.peers_lo = 50;
        args.peers_hi = 30;
        assert!(args.validate().is_err());
    }

    #[test]
    fn test_validate_disabled() {
        let mut args = P2PArgs::default();
        args.p2p_disabled = true;
        args.listen_tcp_port = 9000;
        args.listen_udp_port = 9000;
        assert!(args.validate().is_ok()); // Should not check ports when disabled
    }

    #[test]
    fn test_parse_bootnodes() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(flatten)]
            p2p: P2PArgs,
        }

        let args = vec!["test", "--p2p.bootnodes", "enr1,enr2,enr3"];
        let parsed = TestCli::try_parse_from(args).unwrap();
        assert_eq!(parsed.p2p.bootnodes.len(), 3);
        assert_eq!(parsed.p2p.bootnodes[0], "enr1");
        assert_eq!(parsed.p2p.bootnodes[1], "enr2");
        assert_eq!(parsed.p2p.bootnodes[2], "enr3");
    }
}
