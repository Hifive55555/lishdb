//！ 抽象表及其算子

use std::sync::Arc;

use serde::{Serialize, Deserialize};

use crate::cache::CacheManager;
use crate::expression::DataType;
use crate::storage::SingleValue;

/// 列抽象
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnAbstract {
    pub id: u64,
    pub name: String,
    pub table_name: String,
    pub data_type: DataType,
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

    /// 交
    pub fn and(&self, other: &TableAbstract) -> TableAbstract {
        unimplemented!("and operator is not implemented")
    }

    /// 并
    pub fn or(&self, other: &TableAbstract) -> TableAbstract {
        unimplemented!("or operator is not implemented")
    }

    /// 差
    pub fn minus(&self, other: &TableAbstract) -> TableAbstract {
        unimplemented!("minus operator is not implemented")
    }

    /// 投影
    pub fn project(&self, columns: &[String]) -> TableAbstract {
        unimplemented!("project operator is not implemented")
    }

    /// 选择
    pub fn select(&self, condition: &str) -> TableAbstract {
        unimplemented!("select operator is not implemented")
    }

    /// 排序
    pub fn sort(&self, columns: &[String]) -> TableAbstract {
        unimplemented!("sort operator is not implemented")
    }

    /// 分组
    pub fn group(&self, columns: &[String]) -> TableAbstract {
        unimplemented!("group operator is not implemented")
    }

    /// 聚合
    pub fn aggregate(&self, columns: &[String]) -> TableAbstract {
        unimplemented!("aggregate operator is not implemented")
    }

    /// 连接
    pub fn join(&self, other: &TableAbstract, columns: &[String]) -> TableAbstract {
        unimplemented!("join operator is not implemented")
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
    pub fn to_actual(self, cache: Arc<CacheManager>) -> TableActual {
        let values = self.rows
            .iter()
            .map(|row_handle| {
                // 遍历列，获取对应的值
                self.columns.iter().map(|col| {
                    let value_id = ValueId::new(row_handle.table_id, row_handle.row_id, col.id);
                    cache.get_row(&value_id).unwrap().clone()
                }).collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        TableActual {
            columns: self.columns,
            values,
        }
    }
}
