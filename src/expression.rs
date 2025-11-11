use crate::value::{Column, DataType, RowId, SingleValue, ValueId};
use crate::cache::CacheManager;
use dyn_clone::DynClone;
use log::debug;
use std::cmp::Ordering;

/// 表达式（支持表达式嵌套）
pub(crate) trait Expression : std::fmt::Debug + Send + Sync + DynClone {
    fn data_type(&self) -> Option<DataType>;
    fn is_nullable(&self) -> bool;
    fn children(&self) -> Vec<&Expr>;
    /// 计算表达式的值，返回SingleValue
    fn evaluate(&self, row_id: &RowId, columns: &[Column], cache_manager: &CacheManager) -> SingleValue;
}

/// 主表达式
#[derive(Debug)]
pub struct Expr(pub Box<dyn Expression>);

impl Clone for Expr {
    fn clone(&self) -> Self {
        Self(dyn_clone::clone_box(self.0.as_ref()))
    }
}

impl Default for Expr {
    fn default() -> Self {
        Self(Box::new(ConstantExpr {
            value: String::new(),
            data_type: DataType::Text,
        }))
    }
}

impl Expr {
    pub fn new(expr: Box<dyn Expression>) -> Self {
        Self(expr)
    }

    fn data_type(&self) -> Option<DataType> {
        self.0.data_type()
    }

    fn is_nullable(&self) -> bool {
        self.0.is_nullable()
    }

    fn children(&self) -> Vec<&Expr> {
        self.0.children()
    }

    pub fn evaluate(&self, row_id: &RowId, columns: &[Column], cache_manager: &CacheManager) -> SingleValue {
        self.0.evaluate(row_id, columns, cache_manager)
    }
}

/// 常量表达式
#[derive(Debug, Clone)]
pub(crate) struct ConstantExpr {
    pub value: String,
    pub data_type: DataType,
}

impl Expression for ConstantExpr {
    fn data_type(&self) -> Option<DataType> {
        Some(self.data_type)
    }
    fn is_nullable(&self) -> bool {
        false
    }
    fn children(&self) -> Vec<&Expr> {
        Vec::new()
    }
    fn evaluate(&self, _row_id: &RowId, _columns: &[Column], _cache_manager: &CacheManager) -> SingleValue {
        // 常量表达式直接返回其值的编码形式
        match SingleValue::encode_value(&self.value, &self.data_type) {
            Ok(val) => val,
            Err(_) => SingleValue::NULL
        }
    }
}

/// 标识符表达式（列名或别名）
#[derive(Debug, Clone)]
pub(crate) struct IdentifierExpr {
    pub name: String,
}
impl Expression for IdentifierExpr {
    fn data_type(&self) -> Option<DataType> {
        None // 标识符的类型需要在上下文中解析
    }
    fn is_nullable(&self) -> bool {
        false
    }
    fn children(&self) -> Vec<&Expr> {
        Vec::new()
    }
    fn evaluate(&self, row_id: &RowId, columns: &[Column], cache_manager: &CacheManager) -> SingleValue {
        // 查找对应的列
        if let Some(column) =&columns.iter().find(|col| col.name == self.name) {
            // 创建值ID以从缓存中获取值
            let value_id = ValueId {
                table_id: row_id.table_id,
                row_id: row_id.row_id,
                column_id: column.id
            };
            // 从缓存中获取值，如果不存在则返回NULL
            cache_manager.get_value(&value_id).unwrap_or(SingleValue::NULL)
        } else {
            // 列不存在，返回NULL
            SingleValue::NULL
        }
    }
}

/// 二元运算符
#[derive(Debug, Clone)]
pub(crate) enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    And,
    Or,
}

impl std::fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let op_str = match self {
            BinaryOp::Add => "+",
            BinaryOp::Subtract => "-",
            BinaryOp::Multiply => "*",
            BinaryOp::Divide => "/",
            BinaryOp::Equal => "=",
            BinaryOp::NotEqual => "!=",
            BinaryOp::GreaterThan => ">",
            BinaryOp::GreaterThanOrEqual => ">=",
            BinaryOp::LessThan => "<",
            BinaryOp::LessThanOrEqual => "<=",
            BinaryOp::And => "AND",
            BinaryOp::Or => "OR",
        };
        write!(f, "{}", op_str)
    }
}

/// 二元表达式
#[derive(Debug, Clone)]
pub(crate) struct BinaryExpr {
    pub left: Expr,
    pub op: BinaryOp,
    pub right: Expr,
}

impl Expression for BinaryExpr {
    fn data_type(&self) -> Option<DataType> {
        // 比较和逻辑操作符返回布尔类型
        match self.op {
            BinaryOp::Equal | BinaryOp::NotEqual | BinaryOp::GreaterThan | 
            BinaryOp::GreaterThanOrEqual | BinaryOp::LessThan | BinaryOp::LessThanOrEqual |
            BinaryOp::And | BinaryOp::Or => {
                Some(DataType::Boolean)
            },
            // 算术操作符根据操作数类型确定结果类型
            _ => {
                let left_type = self.left.data_type()?;
                let right_type = self.right.data_type()?;

                // 类型匹配规则
                match (left_type, right_type) {
                    (DataType::Integer, DataType::Integer) => Some(DataType::Integer),
                    (DataType::Float, _) | (_, DataType::Float) => Some(DataType::Float),
                    (DataType::Text, _) | (_, DataType::Text) => Some(DataType::Text),
                    (DataType::Blob, _) | (_, DataType::Blob) => Some(DataType::Blob),
                    (DataType::Boolean, DataType::Boolean) => Some(DataType::Boolean),
                    (DataType::DateTime, DataType::DateTime) => Some(DataType::DateTime),
                    _ => None,
                }
            }
        }
    }
    fn is_nullable(&self) -> bool {
        false
    }
    fn children(&self) -> Vec<&Expr> {
        vec![&self.left, &self.right]
    }
    fn evaluate(&self, row_id: &RowId, columns: &[Column], cache_manager: &CacheManager) -> SingleValue {
        // 计算左右子表达式的值
        let left_value = self.left.0.evaluate(row_id, columns, cache_manager);
        let right_value = self.right.0.evaluate(row_id, columns, cache_manager);
        debug!("BinaryExpr: {} {} {}", left_value, self.op, right_value);

        // 根据操作符类型执行相应的操作
        match self.op {
            // 比较操作符
            BinaryOp::Equal => left_value.equal(&right_value).unwrap_or(false).into(),
            BinaryOp::NotEqual => left_value.not_equal(&right_value).unwrap_or(false).into(),
            BinaryOp::GreaterThan => left_value.greater_than(&right_value).unwrap_or(false).into(),
            BinaryOp::GreaterThanOrEqual => left_value.greater_than_or_equal(&right_value).unwrap_or(false).into(),
            BinaryOp::LessThan => left_value.less_than(&right_value).unwrap_or(false).into(),
            BinaryOp::LessThanOrEqual => left_value.less_than_or_equal(&right_value).unwrap_or(false).into(),
            // 逻辑操作符
            BinaryOp::And => {
                // AND逻辑：左右两边都为true时返回true
                let left_bool = left_value.as_bool().unwrap_or(false);
                let right_bool = right_value.as_bool().unwrap_or(false);
                (left_bool && right_bool).into()
            },
            BinaryOp::Or => {
                // OR逻辑：左右两边有一个为true时返回true
                let left_bool = left_value.as_bool().unwrap_or(false);
                let right_bool = right_value.as_bool().unwrap_or(false);
                (left_bool || right_bool).into()
            },
            // 算术操作符
            BinaryOp::Add => left_value.add(&right_value).unwrap_or(SingleValue::NULL),
            BinaryOp::Subtract => left_value.subtract(&right_value).unwrap_or(SingleValue::NULL),
            BinaryOp::Multiply => left_value.multiply(&right_value).unwrap_or(SingleValue::NULL),
            BinaryOp::Divide => left_value.divide(&right_value).unwrap_or(SingleValue::NULL),
        }
    }
}

/// IN表达式（值在集合中）
#[derive(Debug, Clone)]
pub(crate) struct InExpr {
    pub value: Expr,
    pub values: Vec<Expr>,
}

impl Expression for InExpr {
    fn data_type(&self) -> Option<DataType> {
        Some(DataType::Boolean)
    }
    fn is_nullable(&self) -> bool {
        false
    }
    fn children(&self) -> Vec<&Expr> {
        let mut children = vec![&self.value];
        children.extend(self.values.iter());
        children
    }
    fn evaluate(&self, row_id: &RowId, columns: &[Column], cache_manager: &CacheManager) -> SingleValue {
        // 计算目标值
        let target_value = self.value.evaluate(row_id, columns, cache_manager);
        
        // 检查目标值是否在集合中
        for item in &self.values {
            let item_value = item.evaluate(row_id, columns, cache_manager);
            if target_value.equal(&item_value).unwrap_or(false) {
                return true.into();
            }
        }
        
        false.into()
    }
}

/// 函数类型（函数名）
#[derive(Debug, Clone)]
pub(crate) enum FunctionType {
    LENGTH,
}

/// 函数表达式
#[derive(Debug, Clone)]
pub(crate) struct FunctionExpr {
    pub function_type: FunctionType,
    pub args: Vec<Expr>,
}

impl Expression for FunctionExpr {
    fn data_type(&self) -> Option<DataType> {
        // 根据函数类型和参数类型，判断结果类型
        match self.function_type {
            FunctionType::LENGTH => Some(DataType::Integer),
            _ => unreachable!(),
        }
    }
    fn is_nullable(&self) -> bool {
        false
    }
    fn children(&self) -> Vec<&Expr> {
        self.args.iter().collect()
    }

    fn evaluate(&self, row_id: &RowId, columns: &[Column], cache_manager: &CacheManager) -> SingleValue {
        // 计算函数参数的值
        let arg_values: Vec<SingleValue> = self.args.iter()
            .map(|arg| arg.evaluate(row_id, columns, cache_manager))
            .collect();

        // 根据函数类型执行相应的操作
        match self.function_type {
            FunctionType::LENGTH => {
                // 计算字符串长度
                if let Some(arg) = arg_values.first() {
                    if let DataType::Text = arg.data_type() {
                        let len = arg.as_bytes().len() as i32;
                        return SingleValue::encode_value(&len.to_string(), &DataType::Integer).unwrap_or(SingleValue::NULL);
                    }
                }
                SingleValue::NULL
            },
            _ => unreachable!(),
        }
    }
}
