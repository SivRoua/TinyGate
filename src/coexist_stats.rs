//! Bounded-memory statistics, independent of SDK and clocks.
#[derive(Default)]
pub struct Sequence {
    session: Option<u32>,
    retired: [Option<u32>; 8],
    retired_next: usize,
    high: u32,
    bits: u64,
    pub sessions: u64,
    pub span: u64,
    pub unique: u64,
    pub duplicates: u64,
    pub reordered: u64,
    pub too_old: u64,
}
impl Sequence {
    pub fn add(&mut self, session: u32, seq: u32) {
        if self.session != Some(session) {
            if self.retired.contains(&Some(session)) {
                self.too_old += 1;
                return;
            }
            if let Some(old) = self.session {
                self.retired[self.retired_next] = Some(old);
                self.retired_next = (self.retired_next + 1) % 8;
            }
            self.session = Some(session);
            self.high = seq;
            self.bits = 1;
            self.sessions += 1;
            self.span += 1;
            self.unique += 1;
            return;
        }
        let delta = seq.wrapping_sub(self.high) as i32;
        if delta > 0 {
            self.bits = if delta >= 64 {
                1
            } else {
                (self.bits << delta) | 1
            };
            self.high = seq;
            self.span += delta as u64;
            self.unique += 1;
        } else {
            let back = -(delta as i64);
            if back >= 64 {
                self.too_old += 1;
                return;
            }
            let bit = 1u64 << back;
            if self.bits & bit != 0 {
                self.duplicates += 1;
            } else {
                self.bits |= bit;
                self.unique += 1;
                self.reordered += 1;
            }
        }
    }
    pub fn missing(&self) -> u64 {
        self.span.saturating_sub(self.unique)
    }
}
pub const BOUNDS: [u64; 11] = [1, 5, 10, 25, 50, 100, 250, 500, 1000, 2000, 5000];
#[derive(Default)]
pub struct Latency {
    pub buckets: [u64; 12],
    pub count: u64,
    pub max_ms: u64,
}
impl Latency {
    pub fn add(&mut self, ms: u64) {
        let i = BOUNDS.iter().position(|&v| ms <= v).unwrap_or(11);
        self.buckets[i] += 1;
        self.count += 1;
        self.max_ms = self.max_ms.max(ms);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reordered_packets_fill_gaps_without_duplicate_success() {
        let mut s = Sequence::default();
        s.add(1, 10);
        s.add(1, 12);
        assert_eq!(s.missing(), 1);
        s.add(1, 11);
        s.add(1, 11);
        assert_eq!(
            (s.unique, s.missing(), s.reordered, s.duplicates),
            (3, 0, 1, 1)
        );
    }
    #[test]
    fn reboot_and_counter_wrap() {
        let mut s = Sequence::default();
        s.add(1, u32::MAX);
        s.add(1, 0);
        s.add(2, 0);
        assert_eq!((s.sessions, s.unique, s.missing()), (2, 3, 0));
        s.add(1, 1);
        assert_eq!((s.sessions, s.unique, s.too_old), (2, 3, 1));
    }
    #[test]
    fn expired_window_is_explicit() {
        let mut s = Sequence::default();
        s.add(1, 0);
        s.add(1, 100);
        s.add(1, 1);
        assert_eq!((s.unique, s.too_old, s.missing()), (2, 1, 99));
    }
    #[test]
    fn histogram_boundaries() {
        let mut h = Latency::default();
        h.add(1);
        h.add(5000);
        h.add(5001);
        assert_eq!(
            (
                h.count,
                h.buckets[0],
                h.buckets[10],
                h.buckets[11],
                h.max_ms
            ),
            (3, 1, 1, 1, 5001)
        );
    }
}
