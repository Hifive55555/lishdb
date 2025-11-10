use dyn_clone::DynClone;
use serde::{Serialize, Deserialize};

#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum DataType {
    Unknown,
    Integer,
    Float,
    Text,
    Blob,
    Boolean,
    DateTime,
    String,
    Varchar(usize),
}

/// 表达式（支持表达式嵌套）
pub(crate) trait Expression : std::fmt::Debug + Send + Sync + DynClone {
    fn data_type(&self) -> Option<DataType>;
    fn is_nullable(&self) -> bool;
    fn children(&self) -> Vec<&Expr>;
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
}

/// 二元运算符
#[derive(Debug, Clone)]
pub(crate) enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
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
        // 根据左右子表达式的类型，判断结果类型
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
    fn is_nullable(&self) -> bool {
        false
    }
    fn children(&self) -> Vec<&Expr> {
        vec![&self.left, &self.right]
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
}
