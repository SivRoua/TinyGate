use anyhow::{bail, Result};
pub fn number(text: &str) -> Result<u16> {
    Ok(if let Some(hex) = text.strip_prefix("0x") {
        u16::from_str_radix(hex, 16)?
    } else {
        text.parse()?
    })
}
pub fn address(text: &str) -> Result<[u8; 6]> {
    let pieces: Vec<_> = text.split(':').collect();
    if pieces.len() != 6 {
        bail!("Expected AA:BB:CC:DD:EE:FF");
    }
    let mut result = [0; 6];
    for (i, piece) in pieces.iter().enumerate() {
        if piece.len() != 2 {
            bail!("Invalid address octet");
        }
        result[5 - i] = u8::from_str_radix(piece, 16)?;
    }
    Ok(result)
}
pub fn bytes(text: &str) -> Result<Vec<u8>> {
    if text.is_empty() || text.len() > 256 || text.len() % 2 != 0 || !text.is_ascii() {
        bail!("Expected 1..128 hex bytes");
    }
    (0..text.len())
        .step_by(2)
        .map(|i| Ok(u8::from_str_radix(&text[i..i + 2], 16)?))
        .collect()
}
pub fn credentials(text: &str) -> Result<(&str, &str)> {
    let (ssid, password) = text
        .split_once('|')
        .ok_or_else(|| anyhow::anyhow!("Expected SSID|PASSWORD"))?;
    if ssid.is_empty()
        || ssid.len() > 32
        || password.len() > 63
        || (!password.is_empty() && password.len() < 8)
    {
        bail!("Invalid SSID/password length");
    }
    Ok((ssid, password))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn address_roundtrip_order_and_validation() {
        assert_eq!(
            address("AA:BB:CC:DD:EE:01").unwrap(),
            [1, 0xee, 0xdd, 0xcc, 0xbb, 0xaa]
        );
        assert!(address("AA:BB").is_err());
        assert!(address("GG:00:00:00:00:00").is_err());
    }
    #[test]
    fn binary_and_numeric_limits() {
        assert_eq!(bytes("0001ff").unwrap(), vec![0, 1, 255]);
        assert!(bytes("0").is_err());
        assert!(bytes("é00").is_err());
        assert!(bytes(&"aa".repeat(129)).is_err());
        assert_eq!(number("0xFFFF").unwrap(), 65535);
        assert!(number("65536").is_err());
    }
    #[test]
    fn credentials_allow_spaces_and_explicit_open_network() {
        assert_eq!(
            credentials("My wifi|12345678").unwrap(),
            ("My wifi", "12345678")
        );
        assert_eq!(credentials("Open|").unwrap(), ("Open", ""));
        assert!(credentials("|12345678").is_err());
        assert!(credentials("ssid|short").is_err());
    }
}
