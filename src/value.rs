use crate::error::ValueError;
use std::cmp::Ordering;
use std::fmt::Debug;
use serde::{Serialize, Deserialize};
use log::warn;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
pub enum DataType {
    Null,
    Integer,
    Float,
    Text,
    Blob,
    Boolean,
    DateTime,
    String,
    Varchar(usize),
}

/// 某一列的行的值存储模型
#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq, Clone)]
pub struct SingleValue {
    raw: Vec<u8>,
    data_type: DataType,
}

impl SingleValue {
    pub fn new(raw: Vec<u8>, data_type: DataType) -> Self {
        Self { raw, data_type }
    }

    pub fn len(&self) -> usize {
        self.raw.len()
    }

    pub fn as_ref(&self) -> &[u8] {
        &self.raw
    }

    pub const NULL: Self = Self {
        raw: Vec::new(),
        data_type: DataType::Null,
    };

    /// 根据数据类型编码值
    pub fn encode_value(value: &str, data_type: &DataType) -> Result<Self, ValueError> {
        match data_type {
            DataType::Integer => {
                if let Ok(int_val) = value.parse::<i32>() {
                    Ok(Self {
                        raw: int_val.to_le_bytes().to_vec(),
                        data_type: *data_type,
                    })
                } else {
                    Err(ValueError::ParseError("Failed to parse integer value".to_string()))
                }
            },
            DataType::Float => {
                if let Ok(float_val) = value.parse::<f64>() {
                    Ok(Self {
                        raw: float_val.to_le_bytes().to_vec(),
                        data_type: *data_type,
                    })
                } else {
                    Err(ValueError::ParseError("Failed to parse float value".to_string()))
                }
            },
            DataType::Boolean => {
                let bool_val = value.to_lowercase() == "true" || value == "1";
                Ok(Self {
                    raw: vec![bool_val as u8],
                    data_type: *data_type,
                })
            },
            DataType::String | DataType::Varchar(_) => {
                Ok(Self {
                    raw: value.as_bytes().to_vec(),
                    data_type: *data_type,
                })
            },
            _ => {
                // 其他类型暂时直接存储字符串
                warn!("Unsupported data type {:?} for value {:?}", data_type, value);
                Ok(Self {
                    raw: value.as_bytes().to_vec(),
                    data_type: *data_type,
                })
            }
        }
    }

    pub fn from_str(input: &str) -> Result<Self, ValueError> {
        let trimmed_input = input.trim();
        
        // 解析带引号的字符串
        if let Some(s) = trimmed_input.strip_prefix("'") {
            if let Some(quoted_str) = s.strip_suffix("'") {
                return Self::encode_value(quoted_str, &DataType::String);
            }
        }
        
        // 解析布尔值
        if trimmed_input.eq_ignore_ascii_case("true") || trimmed_input.eq_ignore_ascii_case("false") {
            return Self::encode_value(trimmed_input, &DataType::Boolean);
        }
        
        // 解析浮点数
        if trimmed_input.contains('.') {
            if trimmed_input.parse::<f64>().is_ok() {
                return Self::encode_value(trimmed_input, &DataType::Float);
            }
        }
        
        // 解析整数
        if trimmed_input.parse::<i32>().is_ok() {
            return Self::encode_value(trimmed_input, &DataType::Integer);
        }
        
        // 如果无法识别类型，默认作为字符串处理
        warn!("Cannot determine type for value: {}, treating as string", trimmed_input);
        Self::encode_value(trimmed_input, &DataType::String)
    }

    /// 根据数据类型格式化SingleValue
    pub fn to_string(&self) -> String {
        // 检查是否为空值
        if self.raw.is_empty() {
            return "NULL".to_string();
        }
        
        // 根据数据类型进行转换
        match self.data_type {
            DataType::Integer => {
                if self.raw.len() == 4 {
                    let val = i32::from_le_bytes(self.raw.clone().try_into().unwrap_or([0; 4]));
                    val.to_string()
                } else if self.raw.len() == 8 {
                    let val = i64::from_le_bytes(self.raw.clone().try_into().unwrap_or([0; 8]));
                    val.to_string()
                } else {
                    format!("{:?}", self.raw)
                }
            },
            DataType::Float => {
                if self.raw.len() == 4 {
                    let val = f32::from_le_bytes(self.raw.clone().try_into().unwrap_or([0; 4]));
                    val.to_string()
                } else if self.raw.len() == 8 {
                    let val = f64::from_le_bytes(self.raw.clone().try_into().unwrap_or([0; 8]));
                    val.to_string()
                } else {
                    format!("{:?}", self.raw)
                }
            },
            DataType::Text | DataType::String | DataType::Varchar(_) => {
                // 尝试将二进制数据转换为UTF-8字符串
                match String::from_utf8(self.raw.clone()) {
                    Ok(s) => s,
                    Err(_) => format!("{:?}", self.raw)
                }
            },
            DataType::Boolean => {
                if self.raw.len() >= 1 {
                    (self.raw[0] != 0).to_string()
                } else {
                    "false".to_string()
                }
            },
            DataType::Blob => {
                if self.raw.len() <= 10 {
                    format!("{:?}", self.raw)
                } else {
                    format!("{:?}...", &self.raw[0..10])
                }
            },
            DataType::DateTime => {
                // 假设存储的是Unix时间戳（秒）
                if self.raw.len() == 8 {
                    let timestamp = i64::from_le_bytes(self.raw.clone().try_into().unwrap_or([0; 8]));
                    format!("{}", timestamp)
                } else {
                    format!("{:?}", self.raw)
                }
            },
            DataType::Null => format!("{:?}", self.raw),
        }
    }
}

impl PartialOrd for SingleValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // 先比较数据类型
        if self.data_type != other.data_type {
            return None;
        }
        
        // 数据类型相同，再比较原始值
        self.raw.partial_cmp(&other.raw)
    }
}

impl Ord for SingleValue {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}


/// 列基础信息
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub id: u64,
    pub name: String,
    pub table_name: String,
    pub data_type: DataType,
}

impl Column {
    pub fn same_with(&self, other: &Column) -> bool {
        (self.name == other.name) && (self.data_type == other.data_type)
    }
}


/// 行句柄
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl Debug for RowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 仅显示低3位，方便调试
        write!(f, "{}:{}", self.table_id % 1000, self.row_id % 1000)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColumnId {
    pub table_id: u64,
    pub column_id: u64,
}

impl ColumnId {
    pub fn new(table_id: u64, column_id: u64) -> Self {
        Self { table_id, column_id }
    }
}

impl Debug for ColumnId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 仅显示低3位，方便调试
        write!(f, "{}:{}", self.table_id % 1000, self.column_id % 1000)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl Debug for ValueId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 仅显示低3位，方便调试
        write!(f, "{}:{}:{}", self.table_id % 1000, self.row_id % 1000, self.column_id % 1000)
    }
}
