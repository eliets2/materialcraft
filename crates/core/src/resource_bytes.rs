use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ResourceBytes {
    bytes: Arc<Vec<u8>>,
    hash: u64,
}

impl ResourceBytes {
    /// Take ownership of `bytes` and hash them
    pub fn new(bytes: Vec<u8>) -> ResourceBytes {
        let mut h = DefaultHasher::new();
        bytes.hash(&mut h);
        ResourceBytes { hash: h.finish(), bytes: Arc::new(bytes) }
    }

    /// The bytes
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    /// Number of bytes
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether there are no bytes
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Hash of the whole content
    pub fn content_hash(&self) -> u64 {
        self.hash
    }

    /// Whether both values share one allocation (cheap clones of one file)
    pub fn ptr_eq(&self, other: &ResourceBytes) -> bool {
        Arc::ptr_eq(&self.bytes, &other.bytes)
    }
}

impl Default for ResourceBytes {
    fn default() -> Self {
        ResourceBytes::new(Vec::new())
    }
}

impl PartialEq for ResourceBytes {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash && self.bytes == other.bytes
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn same_length_same_prefix_bytes_hash_differently() {
        let a = vec![7u8; 200];
        let mut b = a.clone();
        b[150] = 8;
        let (a, b) = (ResourceBytes::new(a), ResourceBytes::new(b));
        assert_eq!(a.len(), b.len());
        assert_ne!(a.content_hash(), b.content_hash());
        assert_ne!(a, b);
        assert_eq!(ResourceBytes::default(), ResourceBytes::new(Vec::new()));
    }
}
