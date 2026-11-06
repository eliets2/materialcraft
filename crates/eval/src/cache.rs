use crate::NodeOutput;
use std::collections::HashMap;
use std::sync::Arc;

/// LRU cache of node outputs, bounded by bytes
pub struct Cache {
    entries: HashMap<u64, (Arc<Vec<NodeOutput>>, usize, u64)>,
    bytes: usize,
    cap: usize,
    tick: u64,
    pub(crate) input_hashes: HashMap<usize, crate::input_fingerprint::InputFingerprint>,
}

impl Default for Cache {
    fn default() -> Self {
        Cache::new(1 << 30)
    }
}

impl Cache {
    pub fn new(cap_bytes: usize) -> Cache {
        Cache { entries: HashMap::new(), bytes: 0, cap: cap_bytes, tick: 0, input_hashes: HashMap::new() }
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.input_hashes.clear();
        self.bytes = 0;
    }

    pub(crate) fn get(&mut self, key: u64) -> Option<Arc<Vec<NodeOutput>>> {
        self.tick += 1;
        let t = self.tick;
        self.entries.get_mut(&key).map(|e| {
            e.2 = t;
            e.0.clone()
        })
    }

    pub(crate) fn put(&mut self, key: u64, v: Arc<Vec<NodeOutput>>) {
        let size: usize = v.iter().map(NodeOutput::memory_bytes).sum();
        self.tick += 1;
        if let Some(old) = self.entries.insert(key, (v, size, self.tick)) {
            self.bytes = self.bytes.saturating_sub(old.1);
        }
        self.bytes += size;
        while self.bytes > self.cap && self.entries.len() > 1 {
            let Some((&k, _)) = self.entries.iter().min_by_key(|(_, e)| e.2) else { break };
            if let Some(e) = self.entries.remove(&k) {
                self.bytes = self.bytes.saturating_sub(e.1);
            }
        }
    }
}
