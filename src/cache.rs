use crate::inode::{Inode, InodeId};
use foyer::{Cache, CacheBuilder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const SMALL_FILE_THRESHOLD_BYTES: u64 = 512 * 1024; // 512KB

#[derive(Clone, Hash, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum CacheKey {
    Metadata(InodeId),
    SmallFile(InodeId),
    DirEntry { dir_id: InodeId, name: String },
    Block { inode_id: InodeId, block_index: u64 },
}

#[derive(Clone, Serialize, Deserialize)]
pub enum CacheValue {
    Metadata(#[serde(with = "serde_arc")] Arc<Inode>),
    SmallFile(#[serde(with = "serde_arc")] Arc<Vec<u8>>),
    DirEntry(InodeId),
    Block(#[serde(with = "serde_arc")] Arc<Vec<u8>>),
}

mod serde_arc {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::sync::Arc;

    pub fn serialize<S, T>(arc: &Arc<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        (**arc).serialize(serializer)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Arc<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        Ok(Arc::new(T::deserialize(deserializer)?))
    }
}

#[derive(Clone)]
pub struct UnifiedCache {
    cache: Arc<Cache<Vec<u8>, Vec<u8>>>,
}

impl UnifiedCache {
    pub async fn new(
        _cache_dir: &str,
        _disk_capacity_gb: f64,
        memory_capacity_gb: Option<f64>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // For now, use only in-memory cache to avoid async operation race conditions
        let memory_capacity_bytes =
            (memory_capacity_gb.unwrap_or(0.25) * 1024.0 * 1024.0 * 1024.0) as usize;

        let cache = CacheBuilder::new(memory_capacity_bytes)
            .with_name("zerofs-unified-cache")
            .with_shards(256)
            .with_weighter(|_key: &Vec<u8>, value: &Vec<u8>| -> usize { _key.len() + value.len() })
            .build();

        Ok(Self {
            cache: Arc::new(cache),
        })
    }

    pub async fn get(&self, key: CacheKey) -> Option<CacheValue> {
        let serialized_key = bincode::serialize(&key).ok()?;
        let entry = self.cache.get(&serialized_key)?;
        bincode::deserialize(entry.value()).ok()
    }

    pub fn insert(&self, key: CacheKey, value: CacheValue, _prefer_on_disk: bool) {
        if let (Ok(serialized_key), Ok(serialized_value)) =
            (bincode::serialize(&key), bincode::serialize(&value))
        {
            // For now, ignore prefer_on_disk since we're using in-memory cache only
            self.cache.insert(serialized_key, serialized_value);
        }
    }

    pub fn remove(&self, key: CacheKey) {
        if let Ok(serialized_key) = bincode::serialize(&key) {
            self.cache.remove(&serialized_key);
        }
    }
}
