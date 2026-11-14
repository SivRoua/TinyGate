//! Hardware-independent parser for legacy advertising data.
pub fn local_name(data: &[u8]) -> Option<&[u8]> {
    let mut cursor = 0;
    let mut shortened = None;
    while cursor < data.len() {
        let len = usize::from(data[cursor]);
        if len == 0 {
            break;
        }
        let end = cursor + 1 + len;
        if end > data.len() {
            break;
        }
        match data[cursor + 1] {
            0x09 => return Some(&data[cursor + 2..end]),
            0x08 => shortened = Some(&data[cursor + 2..end]),
            _ => {}
        }
        cursor = end;
    }
    shortened
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_name_after_flags() {
        assert_eq!(
            local_name(&[2, 1, 6, 4, 9, b'S', b'3', b'A']),
            Some(&b"S3A"[..])
        );
    }
    #[test]
    fn complete_name_wins() {
        assert_eq!(
            local_name(&[2, 8, b'A', 3, 9, b'A', b'B']),
            Some(&b"AB"[..])
        );
    }
    #[test]
    fn malformed_payloads_are_safe() {
        for data in [&[][..], &[0, 9], &[5, 9, b'A'], &[1]] {
            assert_eq!(local_name(data), None);
        }
        assert_eq!(local_name(&[2, 8, b'A', 20]), Some(&b"A"[..]));
    }
}
