//! Hardware-independent test verdict: delivery != protocol success != actual state.
pub struct Pending {
    pub id: u32,
    pub desired: bool,
    tx: Option<(u8, bool)>,
    response: Option<(u8, bool)>,
    observed: bool,
}
impl Pending {
    pub fn new(id: u32, desired: bool) -> Self {
        Self {
            id,
            desired,
            tx: None,
            response: None,
            observed: false,
        }
    }
    pub fn tx(&mut self, id: u32, tsn: u8, ok: bool) {
        if id == self.id {
            self.tx = Some((tsn, ok));
        }
    }
    pub fn response(&mut self, tsn: u8, command: u8, ok: bool) {
        if command == u8::from(self.desired) {
            self.response = Some((tsn, ok));
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
        let (response_tsn, response_ok) = self.response?;
        if response_tsn != tsn {
            return None;
        }
        if !response_ok {
            return Some(false);
        }
        self.observed.then_some(true)
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
        p.tx(1, 3, true);
        p.response(3, 1, false);
        assert_eq!(p.verdict(), Some(false));
    }
}
