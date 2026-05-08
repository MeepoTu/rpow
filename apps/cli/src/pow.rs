use sha2::{Digest, Sha256};

pub fn trailing_zero_bits(buf: &[u8]) -> u32 {
    let mut count = 0;
    for byte in buf.iter().rev() {
        if *byte == 0 {
            count += 8;
            continue;
        }
        let mut bit = 0;
        while (byte & (1 << bit)) == 0 {
            bit += 1;
        }
        return count + bit;
    }
    count
}

pub fn verify_solution(prefix: &[u8], nonce: u64, difficulty_bits: u32) -> bool {
    let mut buf = Vec::with_capacity(prefix.len() + 8);
    buf.extend_from_slice(prefix);
    buf.extend_from_slice(&nonce.to_le_bytes());
    let digest = Sha256::digest(buf);
    trailing_zero_bits(&digest) >= difficulty_bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_trailing_zeros() {
        assert_eq!(trailing_zero_bits(&[0b0000_0001]), 0);
        assert_eq!(trailing_zero_bits(&[0b0000_0010]), 1);
        assert_eq!(trailing_zero_bits(&[0b0000_0000, 0b0000_0100]), 2);
    }

    #[test]
    fn verifies_known_solution() {
        let prefix = hex::decode("00112233445566778899aabbccddeeff").unwrap();
        let mut found = None;
        for nonce in 0..1_000_000u64 {
            if verify_solution(&prefix, nonce, 12) {
                found = Some(nonce);
                break;
            }
        }
        assert!(found.is_some());
        assert!(verify_solution(&prefix, found.unwrap(), 12));
    }
}
