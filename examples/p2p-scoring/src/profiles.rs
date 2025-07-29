//! Scoring profiles for different network scenarios
//!
//! This module defines predefined scoring profiles that can be used in different
//! network environments, from permissioned test networks to high-security production
//! deployments.

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Predefined scoring profiles for different network scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScoringProfile {
    /// No peer scoring applied.
    ///
    /// Suitable for:
    /// - Testing and development
    /// - Permissioned networks with trusted peers
    /// - Debugging network connectivity issues
    ///
    /// Characteristics:
    /// - No peer reputation tracking
    /// - No peer banning
    /// - Simplified mesh management
    /// - Higher resource usage due to lack of filtering
    Off,

    /// Light peer scoring with lenient thresholds.
    ///
    /// Suitable for:
    /// - New networks with limited peer diversity
    /// - Networks prioritizing connectivity over security
    /// - Environments with mixed peer quality
    ///
    /// Characteristics:
    /// - Basic peer reputation tracking
    /// - Lenient ban thresholds (-20.0)
    /// - Short ban duration (5 minutes)
    /// - Conservative mesh parameters
    Conservative,

    /// Balanced scoring suitable for most production environments.
    ///
    /// Suitable for:
    /// - Established production networks
    /// - Networks with good peer diversity
    /// - General-purpose deployments
    ///
    /// Characteristics:
    /// - Standard peer reputation system
    /// - Moderate ban thresholds (-40.0)
    /// - Medium ban duration (15 minutes)
    /// - Balanced mesh size for reliability
    Moderate,

    /// Strict scoring for high-security or high-performance requirements.
    ///
    /// Suitable for:
    /// - High-value or security-critical networks
    /// - Networks with many peers and good diversity
    /// - Performance-critical applications
    ///
    /// Characteristics:
    /// - Aggressive peer filtering
    /// - Strict ban thresholds (-60.0)
    /// - Long ban duration (30 minutes)
    /// - Large mesh size for redundancy
    Aggressive,
}

impl ScoringProfile {
    /// Get a description of the scoring profile.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Off => "No peer scoring - suitable for testing and permissioned networks",
            Self::Conservative => "Light scoring with lenient thresholds - good for new networks",
            Self::Moderate => "Balanced scoring suitable for most production environments",
            Self::Aggressive => "Strict scoring for high-security or high-performance requirements",
        }
    }

    /// Get the recommended use cases for this profile.
    pub fn use_cases(&self) -> Vec<&'static str> {
        match self {
            Self::Off => vec![
                "Testing and development",
                "Permissioned networks with trusted peers",
                "Debugging network connectivity issues",
            ],
            Self::Conservative => vec![
                "New networks with limited peer diversity",
                "Networks prioritizing connectivity over security",
                "Gradual rollout of scoring mechanisms",
            ],
            Self::Moderate => vec![
                "Established production networks",
                "Networks with good peer diversity",
                "General-purpose deployments",
            ],
            Self::Aggressive => vec![
                "High-value or security-critical networks",
                "Networks with many peers and good diversity",
                "Performance-critical applications",
            ],
        }
    }

    /// Get the security implications of this profile.
    pub fn security_notes(&self) -> Vec<&'static str> {
        match self {
            Self::Off => vec![
                "No protection against malicious peers",
                "Vulnerable to resource exhaustion attacks",
                "Should only be used in trusted environments",
            ],
            Self::Conservative => vec![
                "Basic protection against obviously malicious peers",
                "May be slow to ban problematic peers",
                "Good balance for networks building reputation",
            ],
            Self::Moderate => vec![
                "Good protection against most attack vectors",
                "Reasonable ban thresholds for production use",
                "Balances security with network participation",
            ],
            Self::Aggressive => vec![
                "Strong protection against malicious behavior",
                "May ban peers aggressively during network issues",
                "Requires careful monitoring to avoid isolation",
            ],
        }
    }

    /// Get performance characteristics of this profile.
    pub fn performance_notes(&self) -> Vec<&'static str> {
        match self {
            Self::Off => vec![
                "Higher bandwidth usage due to lack of filtering",
                "Potentially slower message propagation",
                "No computational overhead from scoring",
            ],
            Self::Conservative => vec![
                "Minimal performance impact from scoring",
                "Good message propagation in most conditions",
                "Lower computational overhead",
            ],
            Self::Moderate => vec![
                "Balanced performance and security tradeoffs",
                "Optimal for most production workloads",
                "Moderate computational overhead",
            ],
            Self::Aggressive => vec![
                "May improve performance by filtering poor peers",
                "Higher computational overhead from scoring",
                "Requires more peers for mesh redundancy",
            ],
        }
    }

    /// Print detailed information about this profile.
    pub fn print_profile_info(&self) {
        println!("=== Scoring Profile: {:?} ===", self);
        println!("{}\n", self.description());

        println!("Use Cases:");
        for use_case in self.use_cases() {
            println!("  • {}", use_case);
        }

        println!("\nSecurity Considerations:");
        for note in self.security_notes() {
            println!("  • {}", note);
        }

        println!("\nPerformance Characteristics:");
        for note in self.performance_notes() {
            println!("  • {}", note);
        }

        println!();
    }
}

impl fmt::Display for ScoringProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => write!(f, "off"),
            Self::Conservative => write!(f, "conservative"),
            Self::Moderate => write!(f, "moderate"),
            Self::Aggressive => write!(f, "aggressive"),
        }
    }
}

impl Default for ScoringProfile {
    fn default() -> Self {
        Self::Conservative
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_display() {
        assert_eq!(ScoringProfile::Off.to_string(), "off");
        assert_eq!(ScoringProfile::Conservative.to_string(), "conservative");
        assert_eq!(ScoringProfile::Moderate.to_string(), "moderate");
        assert_eq!(ScoringProfile::Aggressive.to_string(), "aggressive");
    }

    #[test]
    fn test_profile_descriptions() {
        for profile in [
            ScoringProfile::Off,
            ScoringProfile::Conservative,
            ScoringProfile::Moderate,
            ScoringProfile::Aggressive,
        ] {
            assert!(!profile.description().is_empty());
            assert!(!profile.use_cases().is_empty());
            assert!(!profile.security_notes().is_empty());
            assert!(!profile.performance_notes().is_empty());
        }
    }

    #[test]
    fn test_profile_serialization() {
        for profile in [
            ScoringProfile::Off,
            ScoringProfile::Conservative,
            ScoringProfile::Moderate,
            ScoringProfile::Aggressive,
        ] {
            let json = serde_json::to_string(&profile).unwrap();
            let deserialized: ScoringProfile = serde_json::from_str(&json).unwrap();
            assert_eq!(profile, deserialized);
        }
    }
}
