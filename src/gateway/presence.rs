//! Peer presence: a peer is online while its last sighting is younger than
//! the timeout. This is the bring-up rule (15 s); the per-sensor-period
//! refinement (contracts R22/R26) lands with the device registry.
use embassy_time::{Duration, Instant};

pub struct PresenceTracker {
    timeout: Duration,
    peers: Vec<(u16, Instant)>,
}

impl PresenceTracker {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            peers: Vec::new(),
        }
    }

    /// Record a sighting; `true` when this transitions the peer online.
    pub fn seen(&mut self, address: u16, now: Instant) -> bool {
        for (known, last_seen) in &mut self.peers {
            if *known == address {
                *last_seen = now;
                return false;
            }
        }
        self.peers.push((address, now));
        true
    }

    /// Remove peers whose last sighting aged out; returns the addresses
    /// that transitioned offline in this sweep.
    pub fn sweep(&mut self, now: Instant) -> Vec<u16> {
        let timeout = self.timeout;
        let mut offlines = Vec::new();
        self.peers.retain(|(address, last_seen)| {
            let online = now - *last_seen < timeout;
            if !online {
                offlines.push(*address);
            }
            online
        });
        offlines
    }

    /// Remove a peer explicitly (e.g. Zigbee Leave); `true` if it was online.
    pub fn remove(&mut self, address: u16) -> bool {
        let before = self.peers.len();
        self.peers.retain(|(known, _)| *known != address);
        before != self.peers.len()
    }

    pub fn is_online(&self, address: u16) -> bool {
        self.peers.iter().any(|(known, _)| *known == address)
    }

    pub fn online_count(&self) -> usize {
        self.peers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Default tick is 1 MHz: `Instant::from_ticks` takes microseconds.
    fn at(seconds: u64) -> Instant {
        Instant::from_ticks(seconds * 1_000_000)
    }

    #[test]
    fn sighting_transitions_online_exactly_once() {
        let mut presence = PresenceTracker::new(Duration::from_secs(15));
        assert!(presence.seen(0x1234, at(0)));
        assert!(!presence.seen(0x1234, at(1)));
        assert!(presence.is_online(0x1234));
        assert_eq!(presence.online_count(), 1);
    }

    #[test]
    fn silence_past_the_timeout_reports_offline() {
        let mut presence = PresenceTracker::new(Duration::from_secs(15));
        presence.seen(0x1234, at(10));
        presence.seen(0x4321, at(20));
        assert_eq!(presence.sweep(at(26)), Vec::<u16>::new()); // both fresh
        assert_eq!(presence.sweep(at(31)), vec![0x1234]); // 21 s old now
        assert!(!presence.is_online(0x1234));
        assert!(presence.is_online(0x4321));
    }

    #[test]
    fn a_returning_peer_is_online_again() {
        let mut presence = PresenceTracker::new(Duration::from_secs(15));
        presence.seen(0x1234, at(0));
        presence.sweep(at(16));
        assert!(presence.seen(0x1234, at(100)));
        assert_eq!(presence.sweep(at(105)), Vec::<u16>::new());
    }

    #[test]
    fn explicit_leave_reports_offline_only_when_online() {
        let mut presence = PresenceTracker::new(Duration::from_secs(15));
        presence.seen(0x1234, at(0));
        assert!(presence.remove(0x1234));
        assert!(!presence.remove(0x1234)); // duplicate leave: no phantom event
        assert!(!presence.is_online(0x1234));
    }
}
