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

use linked_hash_map::LinkedHashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use log::{debug, error, warn};
use crate::table::{ColumnId, RowId, ValueId};
use crate::storage::{SingleValue, StorageManager};
use crate::catalog::CatalogManager;
use crate::Result;

/// 带元数据的缓存项包装器
#[derive(Clone)]
pub struct CacheEntry<V> {
    pub data: V,
    access_count: usize,
    last_access_time: Instant,
}

impl<V: Clone> CacheEntry<V> {
    pub fn new(data: V) -> Self {
        Self {
            data,
            access_count: 0,
            last_access_time: Instant::now(),
        }
    }
    
    // 内部方法，更新访问统计和时间
    fn increment_access(&mut self) {
        self.access_count += 1;
        self.last_access_time = Instant::now();
    }
    
    // 检查是否过期
    fn is_expired(&self, ttl: Option<Duration>) -> bool {
        ttl.map_or(false, |t| self.last_access_time.elapsed() > t)
    }
}

/// LRU缓存实现
struct LRUCache<K, V> {
    // 使用LinkedHashMap保持插入顺序，实现LRU策略
    cache: LinkedHashMap<K, CacheEntry<V>>,
    capacity: usize,
    ttl: Option<Duration>,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> LRUCache<K, V> {
    fn new(capacity: usize) -> Self {
        Self::with_ttl(capacity, None)
    }
    
    fn with_ttl(capacity: usize, ttl: Option<Duration>) -> Self {
        Self {
            cache: LinkedHashMap::with_capacity(capacity),
            capacity,
            ttl,
        }
    }
    
    fn insert(&mut self, key: K, value: V) -> Option<V> {
        // 清理过期项
        self.evict_expired();
        
        // 创建缓存条目
        let entry = CacheEntry::new(value);
        
        // 当缓存满时，移除最少使用的项（LinkedHashMap的第一个项）
        if !self.cache.contains_key(&key) && self.cache.len() >= self.capacity {
            if let Some(old_key) = self.cache.keys().next().cloned() {
                self.cache.remove(&old_key);
            }
        }
        
        // 插入新值并返回被替换的值（如果有）
        self.cache.insert(key, entry)
            .map(|old_entry| old_entry.data)
    }
    
    fn get(&mut self, key: &K) -> Option<&V> {
        // 如果键存在
        if let Some(entry) = self.cache.get_mut(key) {
            // 检查是否过期
            if entry.is_expired(self.ttl) {
                // 过期则移除
                self.cache.remove(key);
                return None;
            }
            
            // 更新访问统计
            entry.increment_access();
            
            // 将访问的项移到末尾（表示最近使用）
            if let Some(entry_data) = self.cache.remove(key) {
                let cloned_entry = entry_data.clone();
                self.cache.insert(key.clone(), cloned_entry);
                
                // 返回数据引用
                return self.cache.get(key).map(|e| &e.data);
            }
        }
        None
    }
    
    // 清理过期的缓存项
    fn evict_expired(&mut self) {
        let keys_to_remove: Vec<_> = self.cache.iter()
            .filter(|(_, v)| v.is_expired(self.ttl))
            .map(|(k, _)| k.clone())
            .collect();
            
        for key in keys_to_remove {
            self.cache.remove(&key);
        }
    }
    
    // 手动清理缓存
    fn clear(&mut self) {
        self.cache.clear();
    }
    
    // 获取缓存当前大小
    fn len(&self) -> usize {
        self.cache.len()
    }
}

pub struct SingleCache {
    // 缓存按（表id-记录id-列id）作为唯一标识
    cache: LRUCache<ValueId, SingleValue>,
    capacity: usize,
}

impl SingleCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: LRUCache::new(capacity),
            capacity,
        }
    }

    pub fn with_ttl(capacity: usize, ttl: Duration) -> Self {
        Self {
            cache: LRUCache::with_ttl(capacity, Some(ttl)),
            capacity,
        }
    }

    pub fn insert(&mut self, handle: ValueId, value: SingleValue) -> Option<SingleValue> {
        self.cache.insert(handle, value)
    }

    pub fn get(&mut self, handle: &ValueId) -> Option<&SingleValue> {
        self.cache.get(handle)
    }
    
    // 获取缓存当前大小
    pub fn len(&self) -> usize {
        self.cache.len()
    }
    
    // 手动清理缓存
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}



pub struct CacheManager {
    single_cache: RwLock<SingleCache>,
    storage: Arc<StorageManager>,
    catalog: Arc<CatalogManager>,
}

impl CacheManager {
    pub fn new(storage: Arc<StorageManager>, catalog: Arc<CatalogManager>) -> Self {
        Self {
            single_cache: RwLock::new(SingleCache::new(1000)),
            storage,
            catalog,
        }
    }

    // 创建带TTL的缓存管理器
    pub fn with_ttl(ttl: Duration, storage: Arc<StorageManager>, catalog: Arc<CatalogManager>) -> Self {
        Self {
            single_cache: RwLock::new(SingleCache::with_ttl(1000, ttl)),
            storage,
            catalog,
        }
    }
    
    // 获取行缓存中的值，缓存未命中时会从存储中加载
    pub fn get_row(&self, handle: &ValueId) -> Option<SingleValue> {
        debug!("尝试获取值: {:?}", handle);
        
        // 尝试获取写锁，因为get操作需要更新缓存项的访问统计
        match self.single_cache.write() {
            Ok(mut cache) => {
                // 先检查缓存中是否存在
                if let Some(value) = cache.get(handle).cloned() {
                    debug!("缓存命中: {:?}", handle);
                    return Some(value);
                }
            },
            Err(e) => {
                error!("获取缓存写锁失败: {:?}", e);
                // 即使锁获取失败，我们仍然尝试从存储加载数据
            }
        }
        debug!("缓存未命中: {:?}", handle);
        
        // 缓存未命中，尝试从存储中加载
        // 首先获取表名
        let table_name = match self.catalog.get_table_name_by_id(handle.table_id) {
            Some(name) => {
                debug!("找到表名: {} (table_id={})\n", name, handle.table_id);
                name
            },
            None => {
                warn!("未找到表: table_id={}", handle.table_id);
                return None;
            }
        };
        
        // 然后获取列名
        let column_name = match self.catalog.get_column_name_by_ids(handle.table_id, handle.column_id) {
            Some(name) => {
                debug!("找到列名: {} (column_id={})\n", name, handle.column_id);
                name
            },
            None => {
                warn!("未找到列: table_id={}, column_id={}", handle.table_id, handle.column_id);
                return None;
            }
        };
        
        // 从存储中加载数据
        debug!("从存储中加载: table={}, column={}, row={}", table_name, column_name, handle.row_id);
        match self.storage.get_column_value(&table_name, &column_name, handle.row_id) {
            Ok(Some(value)) => {
                debug!("从存储加载成功");
                // 将加载的值插入缓存
                if let Some(old_value) = self.insert_row(*handle, value.clone()) {
                    debug!("缓存更新，替换旧值");
                } else {
                    debug!("成功插入缓存");
                }
                return Some(value);
            },
            Ok(None) => {
                // 值不存在
                warn!("存储中未找到值: table={}, column={}, row={}", table_name, column_name, handle.row_id);
            },
            Err(e) => {
                error!("从存储加载值失败: {:?}", e);
            }
        }
        
        None
    }
    
    // 插入值到行缓存
    pub fn insert_row(&self, handle: ValueId, value: SingleValue) -> Option<SingleValue> {
        match self.single_cache.write() {
            Ok(mut cache) => {
                let result = cache.insert(handle, value);
                debug!("缓存插入成功: {:?}, 替换旧值: {:?}", handle, result.is_some());
                result
            },
            Err(e) => {
                error!("获取缓存写锁失败，无法插入缓存: {:?}", e);
                None
            }
        }
    }
    
    // 获取缓存当前大小
    pub fn cache_size(&self) -> usize {
        if let Ok(cache) = self.single_cache.read() {
            cache.len()
        } else {
            0
        }
    }
    
    // 清空缓存
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.single_cache.write() {
            cache.clear();
        }
    }
}
