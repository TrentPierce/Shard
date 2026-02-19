use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerHostAnnouncement {
    pub model_id: String,
    pub layer_start: u32,
    pub layer_end: u32,
    pub peer_id: String,
    pub announced_at_ms: u128,
    pub expires_at_ms: u128,
}

#[derive(Debug, Default, Clone)]
pub struct LayerRoutingTable {
    records: HashMap<String, LayerHostAnnouncement>,
}

impl LayerRoutingTable {
    pub fn upsert(&mut self, ann: LayerHostAnnouncement) {
        let key = record_key(&ann.model_id, ann.layer_start, ann.layer_end, &ann.peer_id);
        self.records.insert(key, ann);
    }

    pub fn prune_expired(&mut self, now_ms: u128) {
        self.records.retain(|_, ann| ann.expires_at_ms > now_ms);
    }

    pub fn find_next_layer_peers(
        &self,
        model_id: &str,
        current_layer: u32,
        limit: usize,
    ) -> Vec<String> {
        let next = current_layer.saturating_add(1);
        let mut peers: Vec<String> = self
            .records
            .values()
            .filter(|ann| ann.model_id == model_id && ann.layer_start == next)
            .map(|ann| ann.peer_id.clone())
            .collect();
        peers.sort();
        peers.dedup();
        peers.truncate(limit);
        peers
    }
}

pub fn provider_key(model_id: &str, layer_start: u32) -> libp2p::kad::RecordKey {
    libp2p::kad::RecordKey::new(&format!("shard/layer/{model_id}/{layer_start}"))
}

fn record_key(model_id: &str, layer_start: u32, layer_end: u32, peer_id: &str) -> String {
    format!("{model_id}:{layer_start}:{layer_end}:{peer_id}")
}

#[cfg(test)]
mod tests {
    use super::{LayerHostAnnouncement, LayerRoutingTable};

    #[test]
    fn finds_next_layer_peers() {
        let mut table = LayerRoutingTable::default();
        table.upsert(LayerHostAnnouncement {
            model_id: "m".into(),
            layer_start: 10,
            layer_end: 19,
            peer_id: "peer-a".into(),
            announced_at_ms: 1,
            expires_at_ms: 1000,
        });
        table.upsert(LayerHostAnnouncement {
            model_id: "m".into(),
            layer_start: 10,
            layer_end: 19,
            peer_id: "peer-b".into(),
            announced_at_ms: 1,
            expires_at_ms: 1000,
        });
        let peers = table.find_next_layer_peers("m", 9, 3);
        assert_eq!(peers.len(), 2);
    }
}
