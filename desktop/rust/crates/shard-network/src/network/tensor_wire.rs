use serde::{Deserialize, Serialize};

const MAGIC: [u8; 4] = *b"SFB1";
// DType marker for packed trinary (1.58-bit) payloads.
// Encoding: each trit is 0..2 mapped to base-3 digits.
const DTYPE_TRINARY_PACKED: u8 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TensorWirePacket {
    pub tensor_name: String,
    pub dtype: u8,
    pub shape: Vec<u32>,
    pub data: Vec<u8>,
}

impl TensorWirePacket {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let mut payload = self.data.clone();
        if self.dtype == DTYPE_TRINARY_PACKED {
            if let Some(trit_count) = trit_count_from_shape(&self.shape) {
                let packed_len = packed_trit_len(trit_count);
                if payload.len() == trit_count {
                    payload = pack_trits_5(&payload)?;
                } else if payload.len() != packed_len {
                    return Err(format!(
                        "trinary payload length mismatch: got {}, expected {} or {}",
                        payload.len(),
                        trit_count,
                        packed_len
                    ));
                }
            }
        }

        let mut out = Vec::with_capacity(32 + self.tensor_name.len() + payload.len());
        out.extend_from_slice(&MAGIC);
        out.push(self.dtype);
        out.extend_from_slice(&(self.tensor_name.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.shape.len() as u32).to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(self.tensor_name.as_bytes());
        for d in &self.shape {
            out.extend_from_slice(&d.to_le_bytes());
        }
        out.extend_from_slice(&payload);
        Ok(out)
    }

    pub fn decode(buf: &[u8]) -> Result<Self, String> {
        if buf.len() < 17 || buf[0..4] != MAGIC {
            return Err("invalid tensor wire header".into());
        }

        let dtype = buf[4];
        let name_len = u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]) as usize;
        let shape_len = u32::from_le_bytes([buf[9], buf[10], buf[11], buf[12]]) as usize;
        let data_len = u32::from_le_bytes([buf[13], buf[14], buf[15], buf[16]]) as usize;
        let need = 17 + name_len + (shape_len * 4) + data_len;
        if buf.len() < need {
            return Err("tensor wire payload truncated".into());
        }

        let mut off = 17;
        let name = String::from_utf8(buf[off..off + name_len].to_vec())
            .map_err(|_| "tensor wire invalid UTF-8 name")?;
        off += name_len;

        let mut shape = Vec::with_capacity(shape_len);
        for _ in 0..shape_len {
            let d = u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]);
            shape.push(d);
            off += 4;
        }

        let mut data = buf[off..off + data_len].to_vec();
        if dtype == DTYPE_TRINARY_PACKED {
            if let Some(trit_count) = trit_count_from_shape(&shape) {
                data = unpack_trits_5(&data, trit_count)?;
            }
        }
        Ok(Self {
            tensor_name: name,
            dtype,
            shape,
            data,
        })
    }
}

fn trit_count_from_shape(shape: &[u32]) -> Option<usize> {
    let mut total: u64 = 1;
    for &dim in shape {
        total = total.saturating_mul(dim as u64);
        if total > (usize::MAX as u64) {
            return None;
        }
    }
    Some(total as usize)
}

fn packed_trit_len(trit_count: usize) -> usize {
    (trit_count + 4) / 5
}

fn pack_trits_5(trits: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(packed_trit_len(trits.len()));
    let mut idx = 0;
    while idx < trits.len() {
        let mut value: u8 = 0;
        let mut multiplier: u8 = 1;
        for _ in 0..5 {
            let trit = trits.get(idx).copied().unwrap_or(0);
            if trit > 2 {
                return Err("trinary values must be in range 0..=2".into());
            }
            value = value
                .checked_add(trit.saturating_mul(multiplier))
                .ok_or_else(|| "trinary pack overflow".to_string())?;
            multiplier = multiplier.saturating_mul(3);
            idx = idx.saturating_add(1);
            if idx >= trits.len() {
                break;
            }
        }
        out.push(value);
    }
    Ok(out)
}

fn unpack_trits_5(packed: &[u8], trit_count: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(trit_count);
    for &byte in packed {
        let mut value = byte;
        for _ in 0..5 {
            if out.len() >= trit_count {
                return Ok(out);
            }
            out.push(value % 3);
            value /= 3;
        }
    }
    if out.len() != trit_count {
        return Err("trinary payload truncated".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::TensorWirePacket;

    #[test]
    fn tensor_wire_roundtrip() {
        let pkt = TensorWirePacket {
            tensor_name: "hidden_state".into(),
            dtype: 1,
            shape: vec![2, 4],
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let enc = pkt.encode().expect("encode");
        let dec = TensorWirePacket::decode(&enc).expect("decode");
        assert_eq!(pkt, dec);
    }

    #[test]
    fn trinary_pack_roundtrip() {
        let pkt = TensorWirePacket {
            tensor_name: "trit_weights".into(),
            dtype: super::DTYPE_TRINARY_PACKED,
            shape: vec![2, 3],
            data: vec![0, 1, 2, 2, 1, 0],
        };
        let enc = pkt.encode().expect("encode");
        let dec = TensorWirePacket::decode(&enc).expect("decode");
        assert_eq!(pkt, dec);
    }
}
