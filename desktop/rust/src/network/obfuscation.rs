use sha2::{Digest, Sha256};

const NONCE_BYTES: usize = 12;

pub fn random_nonce() -> [u8; NONCE_BYTES] {
    rand::random::<[u8; NONCE_BYTES]>()
}

pub fn obfuscate_bytes(key: &[u8], nonce: &[u8; NONCE_BYTES], plaintext: &[u8]) -> Vec<u8> {
    xor_stream(key, nonce, plaintext)
}

pub fn deobfuscate_bytes(key: &[u8], nonce: &[u8; NONCE_BYTES], ciphertext: &[u8]) -> Vec<u8> {
    xor_stream(key, nonce, ciphertext)
}

fn xor_stream(key: &[u8], nonce: &[u8; NONCE_BYTES], input: &[u8]) -> Vec<u8> {
    if key.is_empty() {
        return input.to_vec();
    }

    let mut out = vec![0u8; input.len()];
    let mut offset = 0usize;
    let mut counter: u32 = 0;
    while offset < input.len() {
        let mut hasher = Sha256::new();
        hasher.update(key);
        hasher.update(nonce);
        hasher.update(counter.to_le_bytes());
        let block = hasher.finalize();
        let take = (input.len() - offset).min(block.len());
        for i in 0..take {
            out[offset + i] = input[offset + i] ^ block[i];
        }
        offset += take;
        counter = counter.saturating_add(1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{deobfuscate_bytes, obfuscate_bytes, random_nonce};

    #[test]
    fn obfuscation_roundtrip() {
        let key = b"test-key-material";
        let nonce = random_nonce();
        let plain = b"hidden-state-bytes";
        let cipher = obfuscate_bytes(key, &nonce, plain);
        assert_ne!(cipher, plain);
        let out = deobfuscate_bytes(key, &nonce, &cipher);
        assert_eq!(out, plain);
    }
}
