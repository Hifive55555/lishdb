//！ 抽象表及其算子

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use log::{debug, error};

use crate::Result;
use crate::error::TableError;
use crate::cache::CacheManager;
use crate::expression::DataType;
use crate::storage::SingleValue;

/// 列抽象
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnAbstract {
    pub id: u64,
    pub name: String,
    pub table_name: String,
    pub data_type: DataType,
}

impl ColumnAbstract {
    pub fn same_with(&self, other: &ColumnAbstract) -> bool {
        (self.name == other.name) && (self.data_type == other.data_type)
    }
}

/// 行句柄
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RowId {
    pub table_id: u64,
    pub row_id: u64,
}

impl RowId {
    pub fn new(table_id: u64, row_id: u64) -> Self {
        Self { table_id, row_id }
    }

    pub fn get_vec(table_id: u64, row_count: u64) -> Vec<Self> {
        (0..row_count).map(|i| Self::new(table_id, i)).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColumnId {
    pub table_id: u64,
    pub column_id: u64,
}

impl ColumnId {
    pub fn new(table_id: u64, column_id: u64) -> Self {
        Self { table_id, column_id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ValueId {
    pub table_id: u64,
    pub row_id: u64,
    pub column_id: u64,
}

impl ValueId {
    pub fn new(table_id: u64, row_id: u64, column_id: u64) -> Self {
        Self { table_id, row_id, column_id }
    }
}


/// 表抽象
pub struct TableAbstract {
    pub columns: Vec<ColumnAbstract>,
    pub rows: Vec<RowId>,
}

impl TableAbstract {
    // 下面是表抽象的算子

    /// 交集操作 - 返回两个表共有的行
    pub fn and(&self, other: &TableAbstract, cache: Arc<CacheManager>) -> Result<TableAbstract> {
        // 确保表结构兼容
        if self.columns.iter().any(|col| !other.columns.iter().any(|other_col| col.same_with(other_col))) {
            error!("Cannot perform AND operation on tables with different schemas");
            return Err(TableError::SchemaMismatch.into());
        }
        
        // 计算行ID交集
        let self_row_ids: HashSet<_> = self.rows.iter().collect();
        let common_rows: Vec<_> = other.rows.iter()
            .filter(|row| self_row_ids.contains(row))
            .cloned()
            .collect();
        
        Ok(TableAbstract {
            columns: self.columns.clone(),
            rows: common_rows,
        })
    }

    /// 并集操作 - 返回两个表的所有行（去重）
    pub fn or(&self, other: &TableAbstract, cache: Arc<CacheManager>) -> Result<TableAbstract> {
        // 确保表结构兼容
        if self.columns != other.columns {
            error!("Cannot perform OR operation on tables with different schemas");
            return Err(TableError::SchemaMismatch.into());
        }
        
        // 合并行ID并去重
        let mut row_set: HashSet<_> = self.rows.iter().collect();
        let mut all_rows: Vec<_> = self.rows.clone();
        
        for row in &other.rows {
            if !row_set.contains(row) {
                row_set.insert(row);
                all_rows.push(*row);
            }
        }
        
        Ok(TableAbstract {
            columns: self.columns.clone(),
            rows: all_rows,
        })
    }

    /// 差集操作 - 返回在第一个表中但不在第二个表中的行
    pub fn minus(&self, other: &TableAbstract, cache: Arc<CacheManager>) -> Result<TableAbstract> {
        // 确保表结构兼容
        if self.columns != other.columns {
            error!("Cannot perform MINUS operation on tables with different schemas");
            return Err(TableError::SchemaMismatch.into());
        }
        
        // 计算差集
        let other_row_ids: std::collections::HashSet<_> = other.rows.iter().collect();
        let diff_rows: Vec<_> = self.rows.iter()
            .filter(|row| !other_row_ids.contains(row))
            .cloned()
            .collect();
        
        Ok(TableAbstract {
            columns: self.columns.clone(),
            rows: diff_rows,
        })
    }

    /// 投影操作 - 选择指定的列
    pub fn project(&self, column_names: &[String]) -> Result<TableAbstract> {
        // 过滤出需要的列
        let projected_columns: Vec<_> = self.columns.iter()
            .filter(|col| column_names.contains(&col.name))
            .cloned()
            .collect();
        
        // 如果没有找到任何列，返回空表
        if projected_columns.is_empty() {
            error!("None of the specified columns exist in the table");
            return Err(TableError::ColumnNotFound(column_names.join(", ")).into());
        }
        
        Ok(TableAbstract {
            columns: projected_columns,
            rows: self.rows.clone(), // 行引用保持不变，只是列变少了
        })
    }

    /// 选择操作 - 根据条件筛选行
    pub fn select(&self, predicate: impl Fn(&RowId, &[ColumnAbstract], &CacheManager) -> bool, cache: Arc<CacheManager>) -> Result<TableAbstract> {
        // 应用谓词过滤行
        let filtered_rows: Vec<_> = self.rows.iter()
            .filter(|&row_id| predicate(row_id, &self.columns, &cache))
            .cloned()
            .collect();
        
        Ok(TableAbstract {
            columns: self.columns.clone(),
            rows: filtered_rows,
        })
    }

    /// 排序操作 - 根据指定列排序行
    pub fn sort(&self, sort_columns: &[String], cache: Arc<CacheManager>) -> Result<TableAbstract> {
        // 找到排序列的索引和类型
        let column_indices: Vec<_> = sort_columns.iter()
            .filter_map(|col_name| {
                self.columns.iter()
                    .position(|col| col.name == *col_name)
                    .map(|idx| (idx, col_name.clone()))
            })
            .collect();
        
        if column_indices.is_empty() {
            error!("None of the specified sort columns exist in the table");
            return Err(TableError::ColumnNotFound(sort_columns.join(", ")).into());
        }
        
        // 创建可排序的行引用副本
        let mut sortable_rows: Vec<_> = self.rows.clone();
        
        // 排序行
        sortable_rows.sort_by(|&row1, &row2| {
            // 对每个排序列进行比较
            for (col_idx, _) in &column_indices {
                let col = &self.columns[*col_idx];
                
                // 获取两个行在该列的值
                let val_id1 = ValueId::new(row1.table_id, row1.row_id, col.id);
                let val_id2 = ValueId::new(row2.table_id, row2.row_id, col.id);
                
                let val1 = cache.get_row(&val_id1);
                let val2 = cache.get_row(&val_id2);
                
                // 简单比较：如果任一值不存在，空值排在前面
                match (val1, val2) {
                    (None, None) => continue, // 相等，继续比较下一列
                    (None, Some(_)) => return std::cmp::Ordering::Less,
                    (Some(_), None) => return std::cmp::Ordering::Greater,
                    (Some(v1), Some(v2)) => {
                        // 这里简化处理，实际应该根据数据类型进行适当比较
                        let v1_str = String::from_utf8_lossy(&v1.0);
                        let v2_str = String::from_utf8_lossy(&v2.0);
                        let result = v1_str.cmp(&v2_str);
                        if result != std::cmp::Ordering::Equal {
                            return result;
                        }
                    }
                }
            }
            
            // 所有排序列都相等
            std::cmp::Ordering::Equal
        });
        
        Ok(TableAbstract {
            columns: self.columns.clone(),
            rows: sortable_rows,
        })
    }

    /// 聚合操作 - 对表中的数据进行聚合计算
    pub fn aggregate(&self, aggregate_column: &str, cache: Arc<CacheManager>) -> Result<SingleValue> {
        // 找到聚合列
        let target_column = self.columns.iter()
            .find(|col| col.name == *aggregate_column);
            
        if let Some(col) = target_column {
            // 这里实现简单的计数聚合，实际可以根据需要扩展更多聚合函数
            let count = self.rows.len() as u64;
            let count_bytes = count.to_be_bytes().to_vec();
            Ok(SingleValue(count_bytes))
        } else {
            error!("Aggregate column '{}' not found in the table", aggregate_column);
            return Err(TableError::ColumnNotFound(aggregate_column.to_string()).into());
        }
    }
    
    /// 分组操作 - 根据指定列分组
    pub fn group(&self, group_columns: &[String], cache: Arc<CacheManager>) -> Result<Vec<(Vec<SingleValue>, TableAbstract)>> {
        // 找到分组列的索引
        let column_indices: Vec<_> = group_columns.iter()
            .filter_map(|col_name| {
                self.columns.iter()
                    .position(|col| col.name == *col_name)
                    .map(|idx| (idx, &self.columns[idx]))
            })
            .collect();
        
        if column_indices.is_empty() {
            error!("None of the specified group columns exist in the table");
            return Err(TableError::ColumnNotFound(group_columns.join(", ")).into());
        }
        
        // 按分组键存储行
        let mut groups: HashMap<Vec<SingleValue>, Vec<RowId>> = HashMap::new();
        
        // 分组行
        for &row_id in &self.rows {
            // 构建分组键
            let mut key = Vec::new();
            for (_, col) in &column_indices {
                let val_id = ValueId::new(row_id.table_id, row_id.row_id, col.id);
                if let Some(val) = cache.get_row(&val_id) {
                    key.push(val);
                } else {
                    // 如果某个分组列的值不存在，跳过此行
                    key.clear();
                    break;
                }
            }
            
            if !key.is_empty() {
                groups.entry(key.clone()).or_insert_with(Vec::new).push(row_id);
            }
        }
        
        // 转换为结果格式
        Ok(groups.into_iter()
            .map(|(group_key, rows)| {
                (group_key, TableAbstract {
                    columns: self.columns.clone(),
                    rows,
                })
            })
            .collect())
    }

    /// 连接操作 - 根据指定条件连接两个表
    pub fn join(&self, other: &TableAbstract, 
                join_predicate: impl Fn(&RowId, &RowId, &[ColumnAbstract], &[ColumnAbstract], &CacheManager) -> bool, 
                cache: Arc<CacheManager>) -> Result<TableAbstract> {
        // 创建连接后的列（为避免冲突，在其他表的列名前添加前缀）
        let mut joined_columns = self.columns.clone();
        let other_columns_with_prefix: Vec<_> = other.columns.iter()
            .map(|col| ColumnAbstract {
                id: col.id,
                name: format!("other_{}", col.name), // 简单的前缀避免冲突
                table_name: format!("other_{}", col.table_name),
                data_type: col.data_type,
            })
            .collect();
        joined_columns.extend(other_columns_with_prefix);
        
        // 计算笛卡尔积并应用连接条件
        let mut joined_rows = Vec::new();
        let new_table_id = 0; // 临时表ID，实际应该分配新ID
        let mut row_counter = 0;
        
        for &row1 in &self.rows {
            for &row2 in &other.rows {
                if join_predicate(&row1, &row2, &self.columns, &other.columns, &cache) {
                    // 创建新的行引用
                    joined_rows.push(RowId::new(new_table_id, row_counter));
                    row_counter += 1;
                }
            }
        }
        
        Ok(TableAbstract {
            columns: joined_columns,
            rows: joined_rows,
        })
    }
}


/// 表实际
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableActual {
    pub columns: Vec<ColumnAbstract>,
    pub values: Vec<Vec<SingleValue>>,
}

impl TableAbstract {
    /// 将抽象表转换为实际表
    /// 需要从缓存中获取行数据
    pub fn to_actual(self, cache: Arc<CacheManager>) -> Result<TableActual> {
        debug!("开始转换抽象表为实际表，共 {} 行，{} 列", self.rows.len(), self.columns.len());
        
        let mut values = Vec::with_capacity(self.rows.len());
        for (row_idx, row_handle) in self.rows.iter().enumerate() {
            debug!("处理第 {} 行，row_id: {:?}", row_idx, row_handle);
            let mut row_vals = Vec::with_capacity(self.columns.len());
            
            for col in &self.columns {
                let value_id = ValueId::new(row_handle.table_id, row_handle.row_id, col.id);
                debug!("尝试获取列 '{}' (id: {}) 的值，value_id: {:?}", 
                       col.name, col.id, value_id);
                
                match cache.get_row(&value_id) {
                    Some(v) => {
                        debug!("成功获取列 '{}' 的值", col.name);
                        row_vals.push(v);
                    },
                    None => {
                        // 缓存未命中且存储中也未找到值，可能是数据未正确写入或已删除
                        debug!("无法获取列 '{}' 的值，value_id: {:?}", col.name, value_id);
                        // 创建一个空值作为替代，避免整个查询失败
                        row_vals.push(SingleValue(vec![]));
                    }
                }
            }
            
            // 只有当至少有一列有值时才添加此行
            if !row_vals.is_empty() {
                values.push(row_vals);
            }
        }
        
        debug!("成功转换抽象表为实际表，实际包含 {} 行数据", values.len());
        Ok(TableActual {
            columns: self.columns,
            values,
        })
    }
}

impl std::fmt::Display for TableActual {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 如果没有列或行，返回空字符串
        if self.columns.is_empty() || self.values.is_empty() {
            return write!(f, "空表");
        }
        
        // 计算每列的最大宽度
        let mut col_widths: Vec<usize> = self.columns.iter()
            .map(|col| col.name.len())
            .collect();
        
        // 检查每行的值，更新列宽
        for row in &self.values {
            for (i, (val, col)) in row.iter().zip(&self.columns).enumerate() {
                // 转换值为字符串并更新最大宽度
                let val_str = format_value(val, col.data_type);
                col_widths[i] = col_widths[i].max(val_str.len());
            }
        }
        
        // 打印列名
        for (i, col) in self.columns.iter().enumerate() {
            if i > 0 {
                write!(f, " | ")?;
            }
            write!(f, "{:width$}", col.name, width = col_widths[i])?;
        }
        writeln!(f)?;
        
        // 打印分隔线
        for (i, &width) in col_widths.iter().enumerate() {
            if i > 0 {
                write!(f, "-|-").map_err(|_| std::fmt::Error)?;
            }
            write!(f, "{:-<width$}", "", width = width)?;
        }
        writeln!(f)?;
        
        // 打印每一行的值
        for row in &self.values {
            for (i, (val, col)) in row.iter().zip(&self.columns).enumerate() {
                if i > 0 {
                    write!(f, " | ")?;
                }
                let val_str = format_value(val, col.data_type);
                write!(f, "{:width$}", val_str, width = col_widths[i])?;
            }
            writeln!(f)?;
        }
        
        Ok(())
    }
}

/// 根据数据类型格式化SingleValue
/// 这个应该移动到另一个地方
fn format_value(value: &SingleValue, data_type: DataType) -> String {
    // 检查是否为空值
    if value.0.is_empty() {
        return "NULL".to_string();
    }
    
    // 根据数据类型进行转换
    match data_type {
        DataType::Integer => {
            if value.0.len() == 4 {
                let val = i32::from_le_bytes(value.0.clone().try_into().unwrap_or([0; 4]));
                val.to_string()
            } else if value.0.len() == 8 {
                let val = i64::from_le_bytes(value.0.clone().try_into().unwrap_or([0; 8]));
                val.to_string()
            } else {
                format!("{:?}", value.0)
            }
        },
        DataType::Float => {
            if value.0.len() == 4 {
                let val = f32::from_le_bytes(value.0.clone().try_into().unwrap_or([0; 4]));
                val.to_string()
            } else if value.0.len() == 8 {
                let val = f64::from_le_bytes(value.0.clone().try_into().unwrap_or([0; 8]));
                val.to_string()
            } else {
                format!("{:?}", value.0)
            }
        },
        DataType::Text | DataType::String | DataType::Varchar(_) => {
            // 尝试将二进制数据转换为UTF-8字符串
            match String::from_utf8(value.0.clone()) {
                Ok(s) => s,
                Err(_) => format!("{:?}", value.0)
            }
        },
        DataType::Boolean => {
            if value.0.len() >= 1 {
                (value.0[0] != 0).to_string()
            } else {
                "false".to_string()
            }
        },
        DataType::Blob => {
            if value.0.len() <= 10 {
                format!("{:?}", value.0)
            } else {
                format!("{:?}...", &value.0[0..10])
            }
        },
        DataType::DateTime => {
            // 假设存储的是Unix时间戳（秒）
            if value.0.len() == 8 {
                let timestamp = i64::from_le_bytes(value.0.clone().try_into().unwrap_or([0; 8]));
                format!("{}", timestamp)
            } else {
                format!("{:?}", value.0)
            }
        },
        DataType::Unknown => format!("{:?}", value.0),
    }
}

