use std::sync::{Arc, RwLock};
use std::io::Write;
use std::time::Duration;
use log::{trace, debug};

use crate::IndexManager;
use crate::cache::CacheManager;
use crate::error::{Error, Result, TableError};
use crate::storage::{ColumnMeta, StorageManager, TableMeta};
use crate::value::Column;

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

        // 确保表ID基于表名生成
        let expected_id = TableMeta::generate_id(&table_def.name);
        let mut table_def_to_save = table_def.clone();
        
        // 如果ID不一致，自动更新为正确的ID
        if table_def_to_save.id != expected_id {
            trace!("保存时更新表ID: table={}, old_id={}, new_id={}", 
                  table_def_to_save.name, table_def_to_save.id, expected_id);
            table_def_to_save.id = expected_id;
            
            // 同时更新所有列的ID
            for column in &mut table_def_to_save.columns {
                column.id = ColumnMeta::generate_id(expected_id, &column.name);
            }
        }
        
        // 表元数据文件存储在表目录下
        let meta_file_path = table_dir.join("table.meta");
        let mut file = std::fs::File::create(meta_file_path)?;
        let json = serde_json::to_string(&table_def_to_save)?;
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
            let mut table_def: TableMeta = serde_json::from_str(&json)?;
            
            // 确保表ID基于表名生成，保持一致性
            let expected_table_id = TableMeta::generate_id(table_name);
            if table_def.id != expected_table_id {
                // 如果ID不一致，更新为基于名称生成的ID
            trace!("更新表ID以保持一致性: table={}, old_id={}, new_id={}", 
                  table_name, table_def.id, expected_table_id);
            table_def.id = expected_table_id;
                
                // 同时更新所有列的ID，基于新的表ID和列名
                for column in &mut table_def.columns {
                    column.id = ColumnMeta::generate_id(expected_table_id, &column.name);
                }
                
                // 保存更新后的表元数据
                self.save_table_def(&table_def)?;
            }
            
            tables.push(table_def);
        }
        Ok(())
    }
    
    /// 更新表元数据
    pub async fn update_table_meta(&self, updated_meta: TableMeta) -> Result<()> {
        // 获取写锁更新内存中的表元数据
        let mut tables = self.tables.write().unwrap();
        
        // 查找并替换现有的表元数据
        let table_name = &updated_meta.name;
        if let Some(index) = tables.iter().position(|t| t.name == *table_name) {
            tables[index] = updated_meta.clone();
        } else {
            return Err(TableError::TableNotFound(table_name.clone()).into());
        }
        
        // 保存更新后的元数据到文件
        self.save_table_def(&updated_meta)?;
        
        Ok(())
    }
    
    /// 根据表ID查找表名
    pub fn get_table_name_by_id(&self, table_id: u64) -> Option<String> {
        let tables = self.tables.read().unwrap();
        tables.iter()
            .find(|t| t.id == table_id)
            .map(|t| t.name.clone())
    }
    
    /// 根据表ID和列ID查找列名
    pub fn get_column_by_ids(&self, table_id: u64, column_id: u64) -> Option<Column> {
        let tables = self.tables.read().unwrap();
        tables.iter()
            .find(|t| t.id == table_id)
            .and_then(|t| {
                t.columns.iter()
                    .find(|c| c.id == column_id)
                    .map(|c| Column {
                        id: c.id,
                        name: c.name.clone(),
                        table_name: t.name.clone(),
                        data_type: c.data_type,
                    })
            })
    }
}