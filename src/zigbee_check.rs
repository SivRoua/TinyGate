//! Hardware-independent test verdict: delivery != protocol success != actual state.
#[derive(Debug, PartialEq)]
pub enum Poll {
    Waiting,
    Complete(bool),
    Timeout,
    Late(bool),
    Drained,
    Cancelled,
}
#[derive(Debug, PartialEq)]
enum Phase {
    Running,
    Expired,
    Cancelled,
}
#[derive(Debug)]
pub struct Pending {
    pub id: u32,
    pub desired: bool,
    tx: Option<(u8, bool)>,
    // A response may arrive before its TX confirmation identifies the TSN.
    // Keep bounded candidates rather than letting a late response overwrite it.
    responses: [Option<(u8, bool)>; 8],
    response_overflow: bool,
    observed: bool,
    phase: Phase,
}
impl Pending {
    pub fn new(id: u32, desired: bool) -> Self {
        Self {
            id,
            desired,
            tx: None,
            responses: [None; 8],
            response_overflow: false,
            observed: false,
            phase: Phase::Running,
        }
    }
    pub fn tx(&mut self, id: u32, tsn: u8, ok: bool) {
        if id == self.id && self.tx.is_none() {
            self.tx = Some((tsn, ok));
            let matched = self
                .responses
                .iter()
                .flatten()
                .find(|r| r.0 == tsn)
                .copied();
            self.responses = [None; 8];
            self.responses[0] = matched;
        }
    }
    pub fn response(&mut self, tsn: u8, command: u8, ok: bool) {
        if command != u8::from(self.desired)
            || self.tx.is_some_and(|(expected, _)| expected != tsn)
            || self.responses.iter().flatten().any(|r| r.0 == tsn)
        {
            return;
        }
        if let Some(slot) = self.responses.iter_mut().find(|r| r.is_none()) {
            *slot = Some((tsn, ok));
        } else {
            self.response_overflow = true;
        }
    }
    pub fn observed(&mut self, on: bool) {
        self.observed = on == self.desired;
    }
    pub fn verdict(&self) -> Option<bool> {
        let (tsn, ok) = self.tx?;
        if !ok {
            return Some(false);
        }
        let (_, response_ok) = self.responses.iter().flatten().find(|r| r.0 == tsn)?;
        if !*response_ok {
            return Some(false);
        }
        self.observed.then_some(true)
    }
    /// Account a stopped run once, but retain the SDK request until it settles.
    pub fn cancel(&mut self) -> bool {
        if self.phase == Phase::Running {
            self.phase = Phase::Cancelled;
            true
        } else {
            false
        }
    }
    /// A business deadline does not cancel the SDK's asynchronous transmission.
    pub fn poll(&mut self, elapsed_ms: u64, deadline_ms: u64) -> Poll {
        match self.phase {
            Phase::Running if elapsed_ms >= deadline_ms => {
                self.phase = Phase::Expired;
                Poll::Timeout
            }
            Phase::Running => self.verdict().map(Poll::Complete).unwrap_or(Poll::Waiting),
            Phase::Expired => {
                if let Some(ok) = self.verdict() {
                    Poll::Late(ok)
                } else if elapsed_ms >= 10000 && self.tx.is_some() {
                    Poll::Drained
                } else {
                    Poll::Waiting
                }
            }
            Phase::Cancelled => {
                if self.verdict().is_some() || (elapsed_ms >= 10000 && self.tx.is_some()) {
                    Poll::Cancelled
                } else {
                    Poll::Waiting
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delivery_alone_is_not_execution() {
        let mut p = Pending::new(12, true);
        p.tx(12, 4, true);
        assert_eq!(p.verdict(), None);
        p.response(4, 1, true);
        assert_eq!(p.verdict(), None);
        p.observed(true);
        assert_eq!(p.verdict(), Some(true));
    }
    #[test]
    fn accepts_response_before_delivery_callback() {
        let mut p = Pending::new(2, false);
        p.response(9, 0, true);
        p.observed(false);
        assert_eq!(p.verdict(), None);
        p.tx(2, 9, true);
        assert_eq!(p.verdict(), Some(true));
    }
    #[test]
    fn wrong_transaction_or_state_cannot_pass() {
        let mut p = Pending::new(2, false);
        p.tx(99, 5, true);
        p.response(5, 0, true);
        p.observed(false);
        assert_eq!(p.verdict(), None);
        p.tx(2, 6, true);
        assert_eq!(p.verdict(), None);
        p.response(6, 0, true);
        p.observed(true);
        assert_eq!(p.verdict(), None);
    }
    #[test]
    fn failures_do_not_pass_with_matching_state() {
        let mut p = Pending::new(1, true);
        p.observed(true);
        p.tx(1, 3, false);
        assert_eq!(p.verdict(), Some(false));
        let mut p = Pending::new(1, true);
        p.observed(true);
        p.tx(1, 3, true);
        p.response(3, 1, false);
        assert_eq!(p.verdict(), Some(false));
    }
    #[test]
    fn stale_response_cannot_replace_matched_response() {
        let mut p = Pending::new(10, true);
        p.tx(10, 20, true);
        p.response(20, 1, true);
        p.response(19, 1, false); // Late response for an older transaction.
        p.observed(true);
        assert_eq!(p.verdict(), Some(true));
    }
    #[test]
    fn responses_before_confirmation_keep_the_matching_transaction() {
        let mut p = Pending::new(10, true);
        p.response(20, 1, true);
        p.response(19, 1, false);
        p.observed(true);
        p.tx(10, 20, true);
        assert_eq!(p.verdict(), Some(true));
    }
    #[test]
    fn stale_success_cannot_hide_a_matching_failure() {
        let mut p = Pending::new(10, true);
        p.tx(10, 20, true);
        p.response(20, 1, false);
        p.response(19, 1, true);
        p.response(20, 1, true); // Duplicate callback cannot reverse the first result.
        p.observed(true);
        assert_eq!(p.verdict(), Some(false));
    }
    #[test]
    fn duplicate_confirmation_cannot_reverse_failure() {
        let mut p = Pending::new(10, true);
        p.tx(10, 20, false);
        p.tx(10, 20, true);
        p.response(20, 1, true);
        p.observed(true);
        assert_eq!(p.verdict(), Some(false));
    }
    #[test]
    fn bounded_early_responses_do_not_block_later_matching_response() {
        let mut p = Pending::new(10, true);
        for tsn in 0..9 {
            p.response(tsn, 1, true);
        }
        assert!(p.response_overflow);
        p.tx(10, 20, true); // Discard candidates for other transactions.
        p.response(20, 1, true);
        p.observed(true);
        assert_eq!(p.verdict(), Some(true));
    }
    #[test]
    fn deadline_counted_once_and_late_success_is_not_on_time_success() {
        let mut p = Pending::new(10, true);
        assert_eq!(p.poll(2000, 2000), Poll::Timeout);
        assert_eq!(p.poll(3000, 2000), Poll::Waiting);
        p.tx(10, 20, true);
        p.response(20, 1, true);
        p.observed(true);
        assert_eq!(p.poll(3300, 2000), Poll::Late(true));
    }
    #[test]
    fn unresolved_transport_is_retained_even_after_drain_limit() {
        let mut p = Pending::new(10, true);
        assert_eq!(p.poll(2000, 2000), Poll::Timeout);
        assert_eq!(p.poll(15000, 2000), Poll::Waiting);
        p.tx(10, 20, false);
        assert_eq!(p.poll(15001, 2000), Poll::Late(false));
    }
    #[test]
    fn terminal_tx_with_missing_handshake_drains_without_success() {
        let mut p = Pending::new(10, true);
        p.tx(10, 20, true);
        assert_eq!(p.poll(2000, 2000), Poll::Timeout);
        assert_eq!(p.poll(9999, 2000), Poll::Waiting);
        assert_eq!(p.poll(10000, 2000), Poll::Drained);
    }
    #[test]
    fn cancelled_request_is_accounted_once_and_cannot_become_success() {
        let mut p = Pending::new(10, true);
        assert!(p.cancel());
        assert!(!p.cancel());
        assert_eq!(p.poll(3000, 2000), Poll::Waiting);
        p.tx(10, 20, true);
        p.response(20, 1, true);
        p.observed(true);
        assert_eq!(p.poll(3300, 2000), Poll::Cancelled);
    }
}
