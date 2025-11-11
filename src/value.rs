use crate::error::ValueError;
use std::cmp::Ordering;
use std::fmt::Debug;
use chrono::{DateTime, TimeZone, Utc};
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

    pub fn from_slice(raw: &[u8], data_type: DataType) -> Self {
        Self {
            raw: raw.to_vec(),
            data_type,
        }
    }

    pub fn len(&self) -> usize {
        self.raw.len()
    }

    pub fn data_type(&self) -> DataType {
        self.data_type
    }

    pub fn as_bytes(&self) -> &[u8] {
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
            DataType::Integer => self.as_i32().unwrap_or_else(|_| {
                    warn!("Invalid integer value length: {:?}", self.raw);
                    0
                }).to_string(),
            DataType::Float => self.as_f64().unwrap_or_else(|_| {
                    warn!("Invalid float value length: {:?}", self.raw);
                    0.0
                }).to_string(),
            DataType::Text | DataType::String | DataType::Varchar(_) => self.as_str().unwrap_or_else(|e| {
                    warn!("Invalid string value error: {:?}", e);
                    format!("{:?}", self.raw)
                }),
            DataType::Boolean => self.as_bool().unwrap_or_else(|_| {
                    warn!("Invalid boolean value length: {:?}", self.raw);
                    false
                }).to_string(),
            DataType::Blob => {
                if self.raw.len() <= 10 {
                    format!("{:?}", self.raw)
                } else {
                    format!("{:?}...", &self.raw[0..10])
                }
            },
            DataType::DateTime => self.as_datetime().unwrap_or_else(|_| {
                    warn!("Invalid datetime value length: {:?}", self.raw);
                    Utc::now()
                }).format("%Y-%m-%d %H:%M:%S%.f %z").to_string(),
            _ => {
                warn!("Unsupported data type {:?} for value {:?}", self.data_type, self.raw);
                format!("{:?}", self.raw)
            },
        }
    }
    
    pub fn as_i32(&self) -> Result<i32, ValueError> {
        if self.data_type != DataType::Integer {
            return Err(ValueError::TypeMismatch {
                expected: "integer".to_string(),
                found: self.to_string(),
            });
        }
        if self.raw.len() == 4 {
            Ok(i32::from_le_bytes(self.raw.clone().try_into().unwrap_or([0; 4])))
        } else {
            Err(ValueError::InvalidFormat)
        }
    }

    pub fn as_f64(&self) -> Result<f64, ValueError> {
        if self.data_type != DataType::Float {
            return Err(ValueError::TypeMismatch {
                expected: "float".to_string(),
                found: self.to_string(),
            });
        }
        if self.raw.len() == 8 {
            Ok(f64::from_le_bytes(self.raw.clone().try_into().unwrap_or([0; 8])))
        } else {
            Err(ValueError::InvalidFormat)
        }
    }

    pub fn as_bool(&self) -> Result<bool, ValueError> {
        if self.data_type != DataType::Boolean {
            return Err(ValueError::TypeMismatch {
                expected: "boolean".to_string(),
                found: self.to_string(),
            });
        }
        if self.raw.len() >= 1 {
            Ok(self.raw[0] != 0)
        } else {
            Ok(false)
        }
    }

    pub fn as_blob(&self) -> Result<Vec<u8>, ValueError> {
        if self.data_type != DataType::Blob {
            return Err(ValueError::TypeMismatch {
                expected: "blob".to_string(),
                found: self.to_string(),
            });
        }
        Ok(self.raw.clone())
    }

    pub fn as_str(&self) -> Result<String, ValueError> {
        if self.data_type != DataType::String {
            return Err(ValueError::TypeMismatch {
                expected: "string".to_string(),
                found: self.to_string(),
            });
        }
        match String::from_utf8(self.raw.clone()) {
            Ok(s) => Ok(s),
            Err(_) => Err(ValueError::InvalidFormat),
        }
    }

    pub fn as_datetime(&self) -> Result<DateTime<Utc>, ValueError> {
        if self.data_type != DataType::DateTime {
            return Err(ValueError::TypeMismatch {
                expected: "datetime".to_string(),
                found: self.to_string(),
            });
        }
        if self.raw.len() == 8 {
            if let Some(date_time) = DateTime::from_timestamp(
                i64::from_le_bytes(self.raw.clone().try_into().unwrap_or([0; 8])),
                0
            ) {
                Ok(date_time)
            } else {
                Err(ValueError::InvalidFormat)
            }
        } else {
            Err(ValueError::InvalidFormat)
        }
    }
}

impl From<i32> for SingleValue {
    fn from(value: i32) -> Self {
        Self::from_slice(&value.to_le_bytes(), DataType::Integer)
    }
}

impl From<f64> for SingleValue {
    fn from(value: f64) -> Self {
        Self::from_slice(&value.to_le_bytes(), DataType::Float)
    }
}

impl From<bool> for SingleValue {
    fn from(value: bool) -> Self {
        Self::from_slice(&[value as u8], DataType::Boolean)
    }
}

impl From<String> for SingleValue {
    fn from(value: String) -> Self {
        Self::from_slice(value.as_bytes(), DataType::String)
    }
}

impl<T: TimeZone> From<DateTime<T>> for SingleValue {
    fn from(value: DateTime<T>) -> Self {
        Self::from_slice(&value.timestamp().to_le_bytes(), DataType::DateTime)
    }
}

// 这里实现的是基于原始字节值的比较
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

/// 这里实现SingleValue的基本运算
impl SingleValue {
    /// 加法
    pub fn add(&self, other: &Self) -> Result<Self, ValueError> {
        match (self.data_type, other.data_type) {
            // 整数加法
            (DataType::Integer, DataType::Integer) => {
                let a = self.as_i32()?;
                let b = other.as_i32()?;
                Ok(Self::from_slice(&(a + b).to_le_bytes(), DataType::Integer))
            },
            // 浮点数加法
            (DataType::Float, DataType::Float) => {
                let a = self.as_f64()?;
                let b = other.as_f64()?;
                Ok(Self::from_slice(&(a + b).to_le_bytes(), DataType::Float))
            },
            // 整数与浮点数加法
            (DataType::Integer, DataType::Float) => {
                let a = self.as_i32()?;
                let b = other.as_f64()?;
                Ok(Self::from_slice(&(a as f64 + b).to_le_bytes(), DataType::Float))
            },
            // 浮点数与整数加法
            (DataType::Float, DataType::Integer) => {
                let a = self.as_f64()?;
                let b = other.as_i32()?;
                Ok(Self::from_slice(&(a + b as f64).to_le_bytes(), DataType::Float))
            },
            // 字符串加法
            (DataType::String, DataType::String) => {
                let a = self.as_str()?;
                let b = other.as_str()?;
                Ok(Self::from_slice(&(a + &b).into_bytes(), DataType::String))
            },
            // 其他类型加法
            _ => Err(ValueError::TypeMismatch {
                expected: "integer or float or string".to_string(),
                found: self.to_string(),
            }),
        }
    }

    /// 减法
    pub fn subtract(&self, other: &Self) -> Result<Self, ValueError> {
        match (self.data_type, other.data_type) {
            // 整数减法
            (DataType::Integer, DataType::Integer) => {
                let a = self.as_i32()?;
                let b = other.as_i32()?;
                Ok(Self::from_slice(&(a - b).to_le_bytes(), DataType::Integer))
            },
            // 浮点数减法
            (DataType::Float, DataType::Float) => {
                let a = self.as_f64()?;
                let b = other.as_f64()?;
                Ok(Self::from_slice(&(a - b).to_le_bytes(), DataType::Float))
            },
            // 整数与浮点数减法
            (DataType::Integer, DataType::Float) => {
                let a = self.as_i32()?;
                let b = other.as_f64()?;
                Ok(Self::from_slice(&(a as f64 - b).to_le_bytes(), DataType::Float))
            },
            // 浮点数与整数减法
            (DataType::Float, DataType::Integer) => {
                let a = self.as_f64()?;
                let b = other.as_i32()?;
                Ok(Self::from_slice(&(a - b as f64).to_le_bytes(), DataType::Float))
            },
            // 其他类型减法
            _ => Err(ValueError::TypeMismatch {
                expected: "integer or float".to_string(),
                found: self.to_string(),
            }),
        }
    }

    /// 乘法
    pub fn multiply(&self, other: &Self) -> Result<Self, ValueError> {
        match (self.data_type, other.data_type) {
            // 整数乘法
            (DataType::Integer, DataType::Integer) => {
                let a = self.as_i32()?;
                let b = other.as_i32()?;
                Ok(Self::from_slice(&(a * b).to_le_bytes(), DataType::Integer))
            },
            // 浮点数乘法
            (DataType::Float, DataType::Float) => {
                let a = self.as_f64()?;
                let b = other.as_f64()?;
                Ok(Self::from_slice(&(a * b).to_le_bytes(), DataType::Float))
            },
            // 整数与浮点数乘法
            (DataType::Integer, DataType::Float) => {
                let a = self.as_i32()?;
                let b = other.as_f64()?;
                Ok(Self::from_slice(&(a as f64 * b).to_le_bytes(), DataType::Float))
            },
            // 浮点数与整数乘法
            (DataType::Float, DataType::Integer) => {
                let a = self.as_f64()?;
                let b = other.as_i32()?;
                Ok(Self::from_slice(&(a * b as f64).to_le_bytes(), DataType::Float))
            },
            // 其他类型乘法
            _ => Err(ValueError::TypeMismatch {
                expected: "integer or float".to_string(),
                found: self.to_string(),
            }),
        }
    }
    
    /// 除法
    pub fn divide(&self, other: &Self) -> Result<Self, ValueError> {
        match (self.data_type, other.data_type) {
            // 整数除法：若能整除保持整数，否则转为浮点
            (DataType::Integer, DataType::Integer) => {
                let a = self.as_i32()?;
                let b = other.as_i32()?;
                if a % b == 0 {
                    Ok(Self::from_slice(&(a / b).to_le_bytes(), DataType::Integer))
                } else {
                    Ok(Self::from_slice(&((a as f64) / (b as f64)).to_le_bytes(), DataType::Float))
                }
            },
            // 浮点数除法
            (DataType::Float, DataType::Float) => {
                let a = self.as_f64()?;
                let b = other.as_f64()?;
                Ok(Self::from_slice(&(a / b).to_le_bytes(), DataType::Float))
            },
            // 整数与浮点数除法
            (DataType::Integer, DataType::Float) => {
                let a = self.as_i32()?;
                let b = other.as_f64()?;
                Ok(Self::from_slice(&(a as f64 / b).to_le_bytes(), DataType::Float))
            },
            // 浮点数与整数除法
            (DataType::Float, DataType::Integer) => {
                let a = self.as_f64()?;
                let b = other.as_i32()?;
                Ok(Self::from_slice(&(a / b as f64).to_le_bytes(), DataType::Float))
            },
            // 其他类型除法
            _ => Err(ValueError::TypeMismatch {
                expected: "integer or float".to_string(),
                found: self.to_string(),
            }),
        }
    }

    /// 等于
    pub fn equal(&self, other: &Self) -> Result<bool, ValueError> {
        match (self.data_type, other.data_type) {
            (DataType::Integer, DataType::Integer) => {
                let a = self.as_i32()?;
                let b = other.as_i32()?;
                Ok(a == b)
            },
            (DataType::Float, DataType::Float) => {
                let a = self.as_f64()?;
                let b = other.as_f64()?;
                Ok(a == b)
            },
            // 整数与浮点数相等性
            (DataType::Integer, DataType::Float) => {
                let a = self.as_i32()?;
                let b = other.as_f64()?;
                Ok(a as f64 == b)
            },
            // 浮点数与整数相等性
            (DataType::Float, DataType::Integer) => {
                let a = self.as_f64()?;
                let b = other.as_i32()?;
                Ok(a == b as f64)
            },
            // bool
            (DataType::Boolean, DataType::Boolean) => {
                let a = self.as_bool()?;
                let b = other.as_bool()?;
                Ok(a == b)
            },
            // string
            (DataType::String, DataType::String) => {
                let a = self.as_str()?;
                let b = other.as_str()?;
                Ok(a == b)
            },
            // 其他类型相等性
            _ => Err(ValueError::TypeMismatch {
                expected: "integer or float".to_string(),
                found: self.to_string(),
            }),
        }
    }

    /// 不等于
    pub fn not_equal(&self, other: &Self) -> Result<bool, ValueError> {
        self.equal(other).map(|b| !b)
    }

    /// 小于
    /// 仅支持整数和浮点数
    pub fn less_than(&self, other: &Self) -> Result<bool, ValueError> {
        match (self.data_type, other.data_type) {
            (DataType::Integer, DataType::Integer) => {
                let a = self.as_i32()?;
                let b = other.as_i32()?;
                Ok(a < b)
            },
            (DataType::Float, DataType::Float) => {
                let a = self.as_f64()?;
                let b = other.as_f64()?;
                Ok(a < b)
            },
            // 整数与浮点数小于
            (DataType::Integer, DataType::Float) => {
                let a = self.as_i32()?;
                let b = other.as_f64()?;
                Ok((a as f64) < b)
            },
            // 浮点数与整数小于
            (DataType::Float, DataType::Integer) => {
                let a = self.as_f64()?;
                let b = other.as_i32()?;
                Ok(a < (b as f64))
            },
            // 其他类型小于
            _ => Err(ValueError::TypeMismatch {
                expected: "integer or float".to_string(),
                found: self.to_string(),
            }),
        }
    }
    
    /// 小于等于
    pub fn less_than_or_equal(&self, other: &Self) -> Result<bool, ValueError> {
        self.less_than(other).map(|b| b || self.equal(other).unwrap_or(false))
    }

    /// 大于
    pub fn greater_than(&self, other: &Self) -> Result<bool, ValueError> {
        self.less_than_or_equal(other).map(|b| !b)
    }

    /// 大于等于
    pub fn greater_than_or_equal(&self, other: &Self) -> Result<bool, ValueError> {
        self.less_than(other).map(|b| !b)
    }
}

impl std::fmt::Display for SingleValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
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
