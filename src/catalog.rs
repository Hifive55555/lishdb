use std::sync::{Arc, RwLock};
use std::io::Write;
use std::time::Duration;
use log::trace;

use crate::IndexManager;
use crate::cache::CacheManager;
use crate::error::{Error, Result, TableError};
use crate::storage::{StorageManager, TableMeta};

/// 目录管理器
pub struct CatalogManager {
    storage: Arc<StorageManager>,
    tables: Arc<RwLock<Vec<TableMeta>>>,
    // index_manager: Arc<IndexManager>,
}

impl Default for CatalogManager {
    fn default() -> Self {
        Self::new(Arc::new(StorageManager::default())).unwrap()
    }
}


impl CatalogManager {
    /// 创建新的目录管理器
    pub fn new(storage: Arc<StorageManager>) -> Result<Self> {
        let manager = Self {
            storage,
            tables: Arc::new(RwLock::new(Vec::new())),
            // index_manager: Arc::new(IndexManager::default()),
        };
        
        // 加载现有表定义
        manager.load_all_tables()?;
        
        Ok(manager)
    }
    
    /// 创建新表
    pub async fn create_table(&self, table_def: TableMeta) -> Result<()> {
        // 检查表名是否已存在
        if self.has_table(&table_def.name).await? {
            return Err(TableError::TableExists(table_def.name.clone()).into());
        }
        
        // 保存表定义到文件
        self.save_table_def(&table_def)?;

        // 创建空的新表数据文件
        self.storage.create_table_file(&table_def)?;
        
        // 添加到内存
        let mut tables = self.tables.write().unwrap();
        tables.push(table_def);
        
        Ok(())
    }
    
    /// 删除表
    pub async fn drop_table(&self, table_name: &str) -> Result<()> {
        // 先检查表格是否存在
        if !self.has_table(table_name).await? {
            return Err(TableError::TableNotFound(table_name.to_string()).into());
        }
        
        // 从内存中移除
        let mut tables = self.tables.write().unwrap();
        if let Some(index) = tables.iter().position(|t| t.name == table_name) {
            tables.remove(index);
        }
        
        // 删除表定义文件
        let meta_file_path = self.storage.config.data_dir.join(format!("{}.meta", table_name));
        if meta_file_path.exists() {
            std::fs::remove_file(meta_file_path)?;
        }
        
        // 删除表数据文件
        self.storage.delete_table_file(table_name)?;
        
        Ok(())
    }
    
    /// 获取表定义
    pub async fn get_table_meta(&self, table_name: &str) -> Result<Option<TableMeta>> {
        // 虽然这里的实现是同步的，但为了支持异步API，我们保留async关键字
        // 实际实现中，如果需要从磁盘读取，可以使用tokio::task::spawn_blocking
        let tables = self.tables.read().unwrap();
        Ok(tables.iter().find(|t| t.name == table_name).cloned())
    }
    
    /// 检查表是否存在
    pub async fn has_table(&self, table_name: &str) -> Result<bool> {
        // 虽然这里的实现是同步的，但为了支持异步API，我们保留async关键字
        let tables = self.tables.read().unwrap();
        Ok(tables.iter().any(|t| t.name == table_name))
    }
    
    /// 获取所有表名
    pub async fn list_tables(&self) -> Result<Vec<String>> {
        let tables = self.tables.read().unwrap();
        Ok(tables.iter().map(|t| t.name.clone()).collect())
    }
    
    /// 保存表定义到文件
    fn save_table_def(&self, table_def: &TableMeta) -> Result<()> {
        // 确保表目录存在
        let table_dir = self.storage.config.data_dir.join(&table_def.name);
        std::fs::create_dir_all(&table_dir)?;
        
        // 表元数据文件存储在表目录下
        let meta_file_path = table_dir.join("table.meta");
        let mut file = std::fs::File::create(meta_file_path)?;
        let json = serde_json::to_string(table_def)?;
        file.write(json.as_bytes())?;
        Ok(())
    }
    
    /// 加载所有表定义
    fn load_all_tables(&self) -> Result<()> {
        let mut tables = self.tables.write().unwrap();
        tables.clear();
        
        // 遍历data目录下的所有子目录（每个子目录代表一个表）
        for entry in std::fs::read_dir(&self.storage.config.data_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            // 检查是否是目录
            if path.is_dir() {
                if let Some(table_name) = path.file_name().and_then(|os_str| os_str.to_str()) {
                    self.load_table_def(table_name, &mut tables)?;
                }
            }
        }

        trace!("Loaded {} tables' meta file", tables.len());
        Ok(())
    }
    
    /// 加载单个表定义
    fn load_table_def(&self, table_name: &str, tables: &mut Vec<TableMeta>) -> Result<()> {
        // 从表目录下加载表元数据文件
        let meta_file_path = self.storage.config.data_dir.join(table_name).join("table.meta");
        if meta_file_path.exists() {
            let json = std::fs::read_to_string(meta_file_path)?;
            let table_def: TableMeta = serde_json::from_str(&json)?;
            tables.push(table_def);
        }
        Ok(())
    }
}