//! 存储引擎
//! 采用列存储

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};
use chrono::Utc;
use log::{warn};
use crate::DataType;
use crate::error::Result;

/// 列元数据
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ColumnMeta {
    pub id: u64,
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_value: Option<String>,
}

impl Default for ColumnMeta {
    fn default() -> Self {
        // id暂时使用随机代替
        let id = rand::random::<u64>();
        warn!("Column id 暂时使用随机数 {}，后续需要改为自增", id);
        
        Self {
            id,
            name: String::new(),
            data_type: DataType::Unknown,
            nullable: true,
            primary_key: false,
            default_value: None,
        }
    }
}

/// 表元数据
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TableMeta {
    pub id: u64,
    pub name: String,
    pub columns: Vec<ColumnMeta>,
    pub created_at: i64,
    /// 记录计数
    pub row_count: u64,
}

impl TableMeta {
    /// 创建新的表定义
    pub fn new(name: String, columns: Vec<ColumnMeta>) -> Self {
        // id暂时使用随机代替
        let id = rand::random::<u64>();
        warn!("Table id 暂时使用随机数 {}，后续需要改为自增", id);

        Self {
            id,
            name,
            columns,
            created_at: Utc::now().timestamp(),
            row_count: 0,
        }
    }
    
}

/// 某一列的行的值存储模型
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SingleValue(pub Vec<u8>);

/// 存储引擎配置
pub struct StorageConfig {
    pub data_dir: PathBuf,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
        }
    }
}

/// 存储管理器
pub struct StorageManager {
    pub config: StorageConfig,
    file_locks: Arc<Mutex<Vec<(PathBuf, File)>>>,
}

impl Default for StorageManager {
    fn default() -> Self {
        Self::new(StorageConfig::default()).unwrap()
    }
}

impl StorageManager {
    /// 创建新的存储管理器
    pub fn new(config: StorageConfig) -> Result<Self> {
        // 确保数据目录存在
        if !config.data_dir.exists() {
            fs::create_dir_all(&config.data_dir)?;
        }
        
        Ok(Self {
            config,
            file_locks: Arc::new(Mutex::new(Vec::new())),
        })
    }
    
    /// 建表文件
    pub fn create_table_file(&self, table_store: &TableMeta) -> Result<()> {
        // 创建表目录: /data/[table_name]/
        let table_dir = self.config.data_dir.join(&table_store.name);
        std::fs::create_dir_all(&table_dir)?;
        
        // 为每一列创建一个文件
        for column in &table_store.columns {
            // 列文件命名格式: /data/[table_name]/[column_name].col
            let column_file_path = table_dir.join(format!("{}.col", column.name));
            
            // 创建空文件
            let _ = std::fs::File::create(column_file_path)?;
        }
        
        Ok(())
    }
    
    /// 删除表文件
    pub fn delete_table_file(&self, table_name: &str) -> Result<()> {
        // 删除整个表目录: /data/[table_name]/
        let table_dir = self.config.data_dir.join(table_name);
        if table_dir.exists() {
            std::fs::remove_dir_all(table_dir)?;
        }
        
        Ok(())
    }
    
    /// 获取数据库文件列表
    pub fn list_table_files(&self) -> Result<Vec<String>> {
        let mut tables = Vec::new();
        
        for entry in fs::read_dir(&self.config.data_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if let Some(extension) = path.extension() {
                if extension == "tbl" {
                    if let Some(file_name) = path.file_stem() {
                        tables.push(file_name.to_string_lossy().to_string());
                    }
                }
            }
        }
        
        Ok(tables)
    }
}

/// 表文件操作
pub struct TableFile {
    col_files: HashMap<String, File>,
    path: PathBuf,
}

impl TableFile {
    /// 打开表文件
    pub fn open(path: PathBuf) -> Result<Self> {
        unimplemented!("open table file is not implemented")
    }
}
