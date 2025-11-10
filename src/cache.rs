//! 缓存管理器
//! 按表-列-行缓存
//! 
//! 当某个用户操纵一个表时，会为这个表的所有记录创建一个操纵句柄。
//! 当需要访问某个记录时，会拿着该句柄到缓存中查找，这个句柄用（表id-记录id）作为唯一标识。
//! 如果缓存中没有该记录，就会从磁盘中读取，然后缓存起来。
//! 如果缓存中已经有了该记录，就会直接返回。
//! 
//! 索引缓存
//! 
//! 当某个用户查询某个列的值时，会先查询索引缓存，找到对应的行句柄，然后根据行句柄去缓存中查找记录。

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use crate::table::{ColumnId, RowId, ValueId};
use crate::storage::SingleValue;

/// LRU缓存实现
struct LRUCache<K, V> {
    cache: HashMap<K, V>,
    capacity: usize,
}

impl<K: std::hash::Hash + Eq + Clone, V> LRUCache<K, V> {
    fn new(capacity: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(capacity),
            capacity,
        }
    }
    
    fn insert(&mut self, key: K, value: V) -> Option<V> {
        // 当缓存满时，移除最少使用的项
        // 实际实现需要维护访问顺序
        if self.cache.len() >= self.capacity {
            // 简化实现，实际应使用LinkedHashMap或其他数据结构
            self.cache.clear();
        }
        self.cache.insert(key, value)
    }
    
    fn get(&self, key: &K) -> Option<&V> {
        // 访问时更新访问顺序
        self.cache.get(key)
    }
}

pub struct SingleCache {
    // 缓存按（表id-记录id-列id）作为唯一标识
    cache: LRUCache<ValueId, SingleValue>,
}

impl SingleCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: LRUCache::new(capacity),
        }
    }

    pub fn insert(&mut self, handle: ValueId, value: SingleValue) -> Option<SingleValue> {
        self.cache.insert(handle, value)
    }

    pub fn get(&self, handle: &ValueId) -> Option<&SingleValue> {
        self.cache.get(handle)
    }
}

pub struct CacheManager {
    row_cache: SingleCache,
}

impl Default for CacheManager {
    fn default() -> Self {
        Self {
            row_cache: SingleCache::new(1000),
        }
    }
}

impl CacheManager {
    pub fn get_row(&self, handle: &ValueId) -> Option<&SingleValue> {
        self.row_cache.get(handle)
    }
}
