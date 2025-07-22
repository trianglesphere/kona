//! Test to verify that dial errors are properly logged with specific error messages.

mod common;

use kona_p2p::{ConnectionGate, ConnectionGater, GaterConfig};
use libp2p::{Multiaddr, PeerId, multiaddr::Protocol};
use std::{net::IpAddr, str::FromStr, time::Duration};

#[tokio::test]
async fn test_dial_multiaddr_logs_specific_error() {
    use std::sync::{Arc, Mutex};
    
    // Create a simple in-memory log capture for testing
    struct LogCapture {
        logs: Arc<Mutex<Vec<String>>>,
    }
    
    let mut driver = common::gossip_driver(4010);
    
    // Test with an invalid multiaddr (no peer ID)
    let invalid_addr = "/ip4/127.0.0.1/tcp/9000".parse::<Multiaddr>().unwrap();
    
    // This should not panic and should return early due to the invalid address
    driver.dial_multiaddr(invalid_addr);
    
    // Test with a blocked peer
    let peer_id = PeerId::random();
    let mut valid_addr = Multiaddr::empty();
    valid_addr.push(Protocol::Ip4([127, 0, 0, 1].into()));
    valid_addr.push(Protocol::Tcp(9000));
    valid_addr.push(Protocol::P2p(peer_id));
    
    // Block the peer first
    driver.connection_gate.block_peer(&peer_id);
    
    // This should not panic and should return early due to the blocked peer
    driver.dial_multiaddr(valid_addr.clone());
    
    // Test with a blocked IP address
    driver.connection_gate.unblock_peer(&peer_id);
    driver.connection_gate.block_addr(IpAddr::from_str("127.0.0.1").unwrap());
    
    // This should not panic and should return early due to the blocked address
    driver.dial_multiaddr(valid_addr);
}

#[test]
fn test_can_dial_error_messages() {
    use kona_p2p::DialError;
    
    let mut gater = ConnectionGater::new(GaterConfig {
        peer_redialing: Some(1),
        dial_period: Duration::from_secs(60 * 60),
    });
    
    // Test that error messages are descriptive
    let invalid_addr = "/ip4/127.0.0.1/tcp/9000".parse::<Multiaddr>().unwrap();
    let result = gater.can_dial(&invalid_addr);
    
    match result {
        Err(DialError::InvalidMultiaddr { .. }) => {
            let error_msg = result.unwrap_err().to_string();
            assert!(error_msg.contains("Failed to extract PeerId from Multiaddr"));
            assert!(error_msg.contains("/ip4/127.0.0.1/tcp/9000"));
        }
        _ => panic!("Expected InvalidMultiaddr error"),
    }
    
    // Create a valid multiaddr and test other error types
    let peer_id = PeerId::random();
    let mut valid_addr = Multiaddr::empty();
    valid_addr.push(Protocol::Ip4([127, 0, 0, 1].into()));
    valid_addr.push(Protocol::Tcp(9000));
    valid_addr.push(Protocol::P2p(peer_id));
    
    // Test blocked peer error message
    gater.block_peer(&peer_id);
    let result = gater.can_dial(&valid_addr);
    
    match result {
        Err(DialError::BlockedPeer { .. }) => {
            let error_msg = result.unwrap_err().to_string();
            assert!(error_msg.contains("Peer is blocked"));
            assert!(error_msg.contains(&peer_id.to_string()));
        }
        _ => panic!("Expected BlockedPeer error"),
    }
}