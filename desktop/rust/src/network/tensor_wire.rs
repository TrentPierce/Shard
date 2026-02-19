use serde::{Deserialize, Serialize};

const MAGIC: [u8; 4] = *b"SFB1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TensorWirePacket {
    pub tensor_name: String,
    pub dtype: u8,
    pub shape: Vec<u32>,
    pub data: Vec<u8>,
}

impl TensorWirePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + self.tensor_name.len() + self.data.len());
        out.extend_from_slice(&MAGIC);
        out.push(self.dtype);
        out.extend_from_slice(&(self.tensor_name.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.shape.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        out.extend_from_slice(self.tensor_name.as_bytes());
        for d in &self.shape {
            out.extend_from_slice(&d.to_le_bytes());
        }
        out.extend_from_slice(&self.data);
        out
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

        let data = buf[off..off + data_len].to_vec();
        Ok(Self {
            tensor_name: name,
            dtype,
            shape,
            data,
        })
    }
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
        let enc = pkt.encode();
        let dec = TensorWirePacket::decode(&enc).expect("decode");
        assert_eq!(pkt, dec);
    }
}
