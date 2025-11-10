//! 索引管理器
//! 
//! 索引的原理是，为表的每个列创建一个索引文件。
//! 索引文件中存储的是列值和对应的行号。
//! 当需要查询某个列的值时，会先查询索引文件，找到对应的行号，然后根据行号去表文件中读取数据。

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::io::Write;
use serde::{Serialize, Deserialize};
use crate::error::{IndexError, Result, TableError};
use crate::storage::{StorageManager, TableFile};
use crate::catalog::{CatalogManager};
use crate::expression::DataType;

/// 索引类型
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum IndexType {
    BTree,
    Hash,
}

/// 索引定义
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexDef {
    pub name: String,
    pub table_name: String,
    pub column_name: String,
    pub index_type: IndexType,
    pub unique: bool,
}

/// 索引管理器
pub struct IndexManager {
    storage: Arc<StorageManager>,
    catalog: Arc<CatalogManager>,
    indexes: Arc<RwLock<HashMap<String, Arc<dyn Index>>>>,
}

impl IndexManager {
    /// 创建新的索引管理器
    pub fn new(storage: Arc<StorageManager>, catalog: Arc<CatalogManager>) -> Result<Self> {
        let manager = Self {
            storage,
            catalog,
            indexes: Arc::new(RwLock::new(HashMap::new())),
        };
        
        Ok(manager)
    }
    
    /// 创建索引
    pub async fn create_index(&self, index_def: IndexDef) -> Result<()> {
        unimplemented!("create_index");
        // // 检查表是否存在
        // let table_def = self.catalog.get_table_meta(&index_def.table_name).await?
        //     .ok_or_else(|| TableError::TableNotFound(index_def.table_name.clone()))?;
        
        // // 检查列是否存在
        // table_def.find_column(&index_def.column_name)
        //     .ok_or_else(|| ColumnError::ColumnNotFound(index_def.column_name.clone()))?;
        
        // // 创建索引文件
        // let index_file_path = self.storage.config.data_dir.join(format!("{}.idx", index_def.name));
        // std::fs::File::create(index_file_path)?;
        
        // // 创建索引实例（目前使用简化的内存索引）
        // let index: Arc<dyn Index> = match index_def.index_type {
        //     IndexType::BTree => Arc::new(MemoryIndex::new(index_def.clone())),
        //     IndexType::Hash => Arc::new(MemoryIndex::new(index_def.clone())),
        // };
        
        // // 保存索引定义
        // self.save_index_def(&index_def)?;
        
        // // 添加到内存
        // let mut indexes = self.indexes.write().unwrap();
        // indexes.insert(index_def.name.clone(), index);
        
        // Ok(())
    }
    
    /// 删除索引
    pub fn drop_index(&self, index_name: &str) -> Result<()> {
        unimplemented!("drop_index");

        // 从内存中移除
        let mut indexes = self.indexes.write().unwrap();
        if indexes.remove(index_name).is_none() {
            return Err(IndexError::IndexNotFound(index_name.to_string()).into());
        }
        
        // 删除索引文件
        let index_file_path = self.storage.config.data_dir.join(format!("{}.idx", index_name));
        if index_file_path.exists() {
            std::fs::remove_file(index_file_path)?;
        }
        
        // 删除索引定义文件
        let meta_file_path = self.storage.config.data_dir.join(format!("{}.idx.meta", index_name));
        if meta_file_path.exists() {
            std::fs::remove_file(meta_file_path)?;
        }
        
        Ok(())
    }
    
    /// 获取索引
    pub fn get_index(&self, index_name: &str) -> Result<Option<Arc<dyn Index>>> {
        unimplemented!("get_index");

        let indexes = self.indexes.read().unwrap();
        Ok(indexes.get(index_name).cloned())
    }
    
    /// 保存索引定义
    fn save_index_def(&self, index_def: &IndexDef) -> Result<()> {
        unimplemented!("save_index_def");

        let meta_file_path = self.storage.config.data_dir.join(format!("{}.idx.meta", index_def.name));
        let mut file = std::fs::File::create(meta_file_path)?;
        let json = serde_json::to_string(index_def)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }
}

/// 索引接口
pub trait Index: Send + Sync {
    fn name(&self) -> &str;
    fn table_name(&self) -> &str;
    fn column_name(&self) -> &str;
    fn index_type(&self) -> IndexType;
    
    // 添加索引项
    fn insert(&self, key: &str, row_id: u64) -> Result<()>;
    
    // 删除索引项
    fn delete(&self, key: &str, row_id: u64) -> Result<()>;
    
    // 查找索引项
    fn lookup(&self, key: &str) -> Result<Vec<u64>>;
    
    // 范围查询
    fn range_scan(&self, lower: Option<&str>, upper: Option<&str>) -> Result<Vec<(String, u64)>>;
}

/// 简化的内存索引实现（用于演示）
#[derive(Debug)]
pub struct MemoryIndex {
    def: IndexDef,
    data: RwLock<HashMap<String, Vec<u64>>>,
}

impl MemoryIndex {
    pub fn new(def: IndexDef) -> Self {
        Self {
            def,
            data: RwLock::new(HashMap::new()),
        }
    }
}

impl Index for MemoryIndex {
    fn name(&self) -> &str {
        &self.def.name
    }
    
    fn table_name(&self) -> &str {
        &self.def.table_name
    }
    
    fn column_name(&self) -> &str {
        &self.def.column_name
    }
    
    fn index_type(&self) -> IndexType {
        self.def.index_type
    }
    
    fn insert(&self, key: &str, row_id: u64) -> Result<()> {
        let mut data = self.data.write().unwrap();
        let row_ids = data.entry(key.to_string()).or_insert_with(|| Vec::new());
        
        // 如果是唯一索引，检查是否已存在
        if self.def.unique && !row_ids.is_empty() {
            return Err(IndexError::IndexUniqueViolation(format!("唯一索引冲突: 键 '{}' 已存在", key)).into());
        }
        
        row_ids.push(row_id);
        Ok(())
    }
    
    fn delete(&self, key: &str, row_id: u64) -> Result<()> {
        let mut data = self.data.write().unwrap();
        if let Some(row_ids) = data.get_mut(key) {
            if let Some(index) = row_ids.iter().position(|&id| id == row_id) {
                row_ids.remove(index);
                // 如果没有更多行引用，删除该键
                if row_ids.is_empty() {
                    data.remove(key);
                }
            }
        }
        Ok(())
    }
    
    fn lookup(&self, key: &str) -> Result<Vec<u64>> {
        let data = self.data.read().unwrap();
        Ok(data.get(key).cloned().unwrap_or_default())
    }
    
    fn range_scan(&self, lower: Option<&str>, upper: Option<&str>) -> Result<Vec<(String, u64)>> {
        let data = self.data.read().unwrap();
        let mut result = Vec::new();
        
        for (key, row_ids) in data.iter() {
            let in_lower = match lower {
                Some(l) => **key >= *l,
                None => true,
            };
            
            let in_upper = match upper {
                Some(u) => **key <= *u,
                None => true,
            };
            
            if in_lower && in_upper {
                for &row_id in row_ids {
                    result.push((key.clone(), row_id));
                }
            }
        }
        
        // 按键排序
        result.sort_by(|a, b| a.0.cmp(&b.0));
        
        Ok(result)
    }
}