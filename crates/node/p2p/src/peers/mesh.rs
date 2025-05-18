//! Peer mesh management for maintaining stable peer connections.

use libp2p::PeerId;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

/// Default mesh size parameters
pub const DEFAULT_MESH_TARGET: usize = 8;
pub const DEFAULT_MESH_LOW: usize = 6;
pub const DEFAULT_MESH_HIGH: usize = 12;

/// Default mesh rotation period
pub const DEFAULT_ROTATION_PERIOD: Duration = Duration::from_secs(300); // 5 minutes

/// Default percent of peers to rotate when period expires
pub const DEFAULT_ROTATION_PERCENTAGE: f32 = 0.2; // 20%

/// Default grace period for a newly connected peer before considering it for rotation
pub const DEFAULT_GRACE_PERIOD: Duration = Duration::from_secs(60); // 1 minute

/// Manages peer mesh with stable targeted size and controlled rotation
#[derive(Debug, Clone)]
pub struct MeshManager {
    /// Target number of peers to maintain
    pub target: usize,
    /// Low watermark - below this we'll actively seek more peers
    pub low_watermark: usize,
    /// High watermark - above this we may prune connections
    pub high_watermark: usize,
    /// How often to rotate peers (actively disconnect some peers to refresh mesh)
    pub rotation_period: Duration,
    /// What percentage of peers to rotate each period
    pub rotation_percentage: f32,
    /// Grace period for a peer before considering it for rotation
    pub grace_period: Duration,
}

impl Default for MeshManager {
    fn default() -> Self {
        Self {
            target: DEFAULT_MESH_TARGET,
            low_watermark: DEFAULT_MESH_LOW,
            high_watermark: DEFAULT_MESH_HIGH,
            rotation_period: DEFAULT_ROTATION_PERIOD,
            rotation_percentage: DEFAULT_ROTATION_PERCENTAGE,
            grace_period: DEFAULT_GRACE_PERIOD,
        }
    }
}

impl MeshManager {
    /// Create a new mesh manager with custom parameters
    pub const fn new(
        target: usize,
        low_watermark: usize,
        high_watermark: usize,
        rotation_period: Duration,
        rotation_percentage: f32,
        grace_period: Duration,
    ) -> Self {
        Self {
            target,
            low_watermark,
            high_watermark,
            rotation_period,
            rotation_percentage,
            grace_period,
        }
    }
}

/// Tracker for peer connection times and rotation state
#[derive(Debug)]
pub struct MeshTracker {
    /// Last time the mesh was rotated
    pub last_rotation: Instant,
    /// Map of peer IDs to their connection time
    pub peer_connection_times: HashMap<PeerId, Instant>,
    /// Peers that have been selected for rotation but not yet disconnected
    pub peers_to_rotate: Vec<PeerId>,
    /// Mesh management configuration
    pub config: MeshManager,
}

impl MeshTracker {
    /// Creates a new mesh tracker with the given configuration
    pub fn new(config: MeshManager) -> Self {
        Self {
            last_rotation: Instant::now(),
            peer_connection_times: HashMap::new(),
            peers_to_rotate: Vec::new(),
            config,
        }
    }

    /// Tracks a newly connected peer
    pub fn peer_connected(&mut self, peer_id: PeerId) {
        self.peer_connection_times.insert(peer_id, Instant::now());
    }

    /// Removes a disconnected peer from tracking
    pub fn peer_disconnected(&mut self, peer_id: &PeerId) {
        self.peer_connection_times.remove(peer_id);
        self.peers_to_rotate.retain(|p| p != peer_id);
    }

    /// Gets the total count of peers in the mesh
    pub fn peer_count(&self) -> usize {
        self.peer_connection_times.len()
    }

    /// Whether the mesh has a healthy number of peers
    pub fn is_mesh_healthy(&self) -> bool {
        let count = self.peer_count();
        count >= self.config.low_watermark && count <= self.config.high_watermark
    }

    /// Whether the mesh needs more peers
    pub fn needs_more_peers(&self) -> bool {
        self.peer_count() < self.config.low_watermark
    }

    /// Whether the mesh has too many peers
    pub fn has_excess_peers(&self) -> bool {
        self.peer_count() > self.config.high_watermark
    }

    /// Checks if it's time to rotate peers in the mesh
    pub fn should_rotate_peers(&self) -> bool {
        self.last_rotation.elapsed() >= self.config.rotation_period
    }

    /// Returns a list of peers to disconnect for rotation
    ///
    /// This method selects the oldest peers that are past the grace period
    /// up to the configured rotation percentage.
    pub fn select_peers_for_rotation(&mut self) {
        // Reset rotation timer
        self.last_rotation = Instant::now();

        // If we have no peers or too few peers, don't rotate
        if self.peer_count() < self.config.target {
            return;
        }

        // Calculate how many peers to rotate
        let rotation_count =
            ((self.peer_count() as f32) * self.config.rotation_percentage).ceil() as usize;
        if rotation_count == 0 {
            return;
        }

        // Select oldest peers that are beyond grace period
        let now = Instant::now();
        let mut eligible_peers: Vec<_> = self
            .peer_connection_times
            .iter()
            .filter(|(_, connected_at)| {
                now.duration_since(**connected_at) > self.config.grace_period
            })
            .collect();

        eligible_peers.sort_by(|(_, a_time), (_, b_time)| a_time.cmp(b_time));

        // Take the oldest 'rotation_count' peers
        let peers_to_rotate = eligible_peers
            .into_iter()
            .take(rotation_count)
            .map(|(peer_id, _)| *peer_id)
            .collect::<Vec<_>>();

        // Store the rotation list (used by other methods)
        self.peers_to_rotate = peers_to_rotate;
    }
}
