//! 存储引擎
//! 采用列存储

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};
use chrono::Utc;
use log::{debug, error, trace, warn};
use crate::value::{Column, DataType, SingleValue};
use crate::error::{Result, StorageError};

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

impl ColumnMeta {
    /// 基于表ID和列名生成持久化的列ID
    pub fn generate_id(table_id: u64, column_name: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        table_id.hash(&mut hasher);
        column_name.hash(&mut hasher);
        hasher.finish()
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
    /// 基于表名生成持久化的表ID
    pub fn generate_id(table_name: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        table_name.hash(&mut hasher);
        hasher.finish()
    }
    
    /// 创建新的表定义
    pub fn new(name: String, columns: Vec<ColumnMeta>) -> Self {
        // 基于表名生成持久化的ID
        let id = Self::generate_id(&name);
        
        // 确保每个列都有基于表ID和列名生成的持久化ID
        let mut columns_with_id = columns.clone();
        for column in &mut columns_with_id {
            if column.id == 0 { // 如果列ID未设置
                column.id = ColumnMeta::generate_id(id, &column.name);
            }
        }

        Self {
            id,
            name,
            columns: columns_with_id,
            created_at: Utc::now().timestamp(),
            row_count: 0,
        }
    }
}

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
    
    /// 从指定表的指定列的指定行读取值
    pub fn get_column_value(&self, table_name: &str, column: &Column, row_index: u64) -> Result<Option<SingleValue>> {
        let table_path = self.config.data_dir.join(table_name);
        
        // 检查表目录是否存在
        if !table_path.exists() {
            warn!("表目录不存在: {:?}", table_path);
            return Ok(None);
        }
        
        // 打开表文件（传入表目录路径）
        let mut table_file = TableFile::open(table_path)?;
        
        // 读取指定列的指定行
        let result = table_file.read_column_value(column, row_index)?;
        
        // 关闭表文件
        table_file.close()?;
        
        trace!("从表 {} 的列 {} 的第 {} 行读取值: {:?}", table_name, column.name, row_index, result);
        Ok(result)
    }
    
    /// 向表中插入数据
    pub fn insert_data(&self, table_meta: &TableMeta, column_names: &[String], values: &[Vec<String>]) -> Result<()> {
        trace!("向表 {} 插入 {} 行数据", table_meta.name, values.len());

        let table_path = self.config.data_dir.join(&table_meta.name);
        
        // 检查表目录是否存在
        if !table_path.exists() {
            warn!("表目录不存在: {:?}", table_path);
            return Err(crate::error::TableError::TableNotFound(table_meta.name.clone()).into());
        }
        
        // 打开表文件（传入表目录路径）
        let mut table_file = TableFile::open(table_path)?;
        
        // 将表元数据传递给 TableFile 进行操作
        table_file.insert_data(table_meta, column_names, values)?;
        
        Ok(())
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
        if !path.exists() {
            return Err(crate::error::TableError::TableNotFound(path.display().to_string()).into());
        }
        
        // 初始化空的列文件映射，在需要时动态打开
        let col_files = HashMap::new();
        
        Ok(Self {
            col_files,
            path,
        })
    }
    
    /// 向表中插入数据
    pub fn insert_data(&mut self, table_meta: &TableMeta, column_names: &[String], values: &[Vec<String>]) -> Result<()> {
        trace!("开始插入数据，共 {} 行，每行 {} 列", values.len(), column_names.len());
        
        // 对每一行数据
        for (row_idx, row_values) in values.iter().enumerate() {
            trace!("处理第 {} 行数据", row_idx);
            
            // 对每一列数据
            for (col_idx, col_name) in column_names.iter().enumerate() {
                trace!("处理第 {} 行的列: {}", row_idx, col_name);
                
                // 找到对应的列元数据
                let column_meta = table_meta.columns.iter()
                    .find(|col| col.name == *col_name)
                    .ok_or_else(|| crate::error::TableError::ColumnNotFound(col_name.clone()))?;
                
                // 获取列文件并确保已打开
                let file = self.get_or_open_column_file(col_name)?;
                
                // 根据数据类型进行编码并写入
                let value = &row_values[col_idx];
                trace!("编码值: {} (类型: {:?})", value, column_meta.data_type);
                let encoded_value = SingleValue::encode_value(value, &column_meta.data_type)?;
                
                // 写入值的长度作为前缀
                let len_bytes = encoded_value.len() as u32;
                trace!("写入长度前缀: {}", len_bytes);
                file.write_all(&len_bytes.to_le_bytes())?;
                
                // 写入实际值
                trace!("写入实际值，长度: {}", encoded_value.len());
                file.write_all(encoded_value.as_ref())?;
                
                // 刷新缓冲区到磁盘，确保数据持久化
                file.flush()?;
            }
        }
        
        trace!("数据插入完成");
        Ok(())
    }
    
    /// 获取或打开列文件
    fn get_or_open_column_file(&mut self, column_name: &str) -> Result<&mut File> {
        // 如果文件已经打开，直接返回
        if self.col_files.contains_key(column_name) {
            trace!("列文件 {} 已打开，直接返回", column_name);
            // 使用unwrap是安全的，因为我们已经检查了键存在
            return Ok(self.col_files.get_mut(column_name).unwrap());
        }
        
        // 构建列文件路径
        let column_file_path = self.path.join(format!("{}.col", column_name));
        
        // 确保父目录存在
        if let Some(parent) = column_file_path.parent() {
            if !parent.exists() {
                trace!("创建父目录: {:?}", parent);
                std::fs::create_dir_all(parent)?;
            }
        }
        
        // 打开列文件，对于读取操作需要read权限，对于写入操作需要append权限
        let file = OpenOptions::new()
            .read(true)  // 添加读取权限
            .write(true) // 添加写入权限
            .append(true)
            .create(true)
            .open(column_file_path)?;
        
        // 存储到映射中
        let column_name_str = column_name.to_string();
        self.col_files.insert(column_name_str.clone(), file);
        
        // 返回新打开的文件
        Ok(self.col_files.get_mut(&column_name_str).unwrap())
    }
    
    /// 从指定列中读取一行数据
    pub fn read_column_value(&mut self, column: &Column, row_index: u64) -> Result<Option<SingleValue>> {
        trace!("读取列 {} 的第 {} 行数据", column.name, row_index);
        
        // 确保列文件已打开
        let file = self.get_or_open_column_file(&column.name)?;
        
        // 重置文件指针到开始位置
        let seek_result = file.seek(SeekFrom::Start(0));
        if let Err(e) = seek_result {
            warn!("重置文件指针失败: {:?}", e);
            return Err(e.into());
        }
        
        // 遍历查找指定行
        for i in 0..row_index {
            // 读取值的长度
            let mut len_bytes = [0; 4];
            match file.read_exact(&mut len_bytes) {
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // 文件过早结束，说明没有更多行
                    warn!("读取行 {} 时文件过早结束", i);
                    return Ok(None);
                },
                Err(e) => {
                    warn!("读取行 {} 的长度失败: {:?}", i, e);
                    return Err(e.into());
                },
                Ok(_) => {}
            }
            
            let len = u32::from_le_bytes(len_bytes) as i64;
            
            // 跳过值内容
            if let Err(e) = file.seek(SeekFrom::Current(len)) {
                warn!("跳过行 {} 的内容失败: {:?}", i, e);
                return Err(e.into());
            }
        }
        
        // 读取目标行的值长度
        let mut len_bytes = [0; 4];
        match file.read_exact(&mut len_bytes) {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                // 文件过早结束，目标行不存在
                warn!("读取目标行 {} 的长度时文件过早结束", row_index);
                return Ok(None);
            },
            Err(e) => {
                warn!("读取目标行 {} 的长度失败: {:?}", row_index, e);
                return Err(e.into());
            },
            Ok(_) => {}
        }
        
        let len = u32::from_le_bytes(len_bytes) as usize;
        // debug!("目标行 {} 的值长度为: {}", row_index, len);
        
        // 读取值内容
        let mut buffer = vec![0; len];
        match file.read_exact(&mut buffer) {
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                // 文件过早结束，值不完整
                warn!("读取目标行 {} 的内容时文件过早结束", row_index);
                return Ok(None);
            },
            Err(e) => {
                warn!("读取目标行 {} 的内容失败: {:?}", row_index, e);
                return Err(e.into());
            },
            Ok(_) => {}
        }
        
        trace!("成功读取目标行 {} 的值", row_index);
        Ok(Some(SingleValue::new(buffer, column.data_type)))
    }
    
    /// 删除表中的某一行数据（简化实现：标记删除，不实际物理删除）
    pub fn delete_row(&mut self, _row_index: u64) -> Result<()> {
        // 实际实现中，可能需要维护一个删除标记文件或位图
        // 这里简化实现，后续可以扩展
        Ok(())
    }
    
    /// 关闭所有打开的文件
    pub fn close(&mut self) -> Result<()> {
        // 清空映射，文件会自动关闭
        self.col_files.clear();
        Ok(())
    }
}
