use crate::expression::{DataType, Expr};

/// SQL语句解析结果枚举
#[derive(Debug, Clone)]
pub enum Stmt {
    Select(SelectStmt),
    Create(CreateStmt),
    Drop(DropStmt),
    ShowTables(ShowTablesStmt),
    Insert(InsertStmt),
    // 其他语句类型可以在这里添加
}

/// SQL语句的最高层类型
#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum StmtType {
    Select,
    Create,
    Insert,
    Update,
    Delete,
    Drop,
    ShowTables,
}

#[derive(Debug, Clone)]
pub(crate) struct SelectStmt {
    pub columns: Vec<ColumnStmt>,
    pub table: TableStmt,
    pub where_expr: Option<Expr>,
}

/// 列约束
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnConstraint {
    PrimaryKey,
    Nullable,
    NotNull,
    // 可以添加更多约束类型
}

/// 列
#[derive(Debug, PartialEq, Clone)]
pub(crate) struct ColumnStmt {
    pub name: String,
    pub alias: Option<String>,
    pub data_type: DataType,
    pub constraints: Vec<ColumnConstraint>,
    pub default_value: Option<String>,
}

impl ColumnStmt {
    pub fn new(name: &str) -> Self {
        ColumnStmt {
            name: name.to_string(),
            alias: None,
            data_type: DataType::Unknown,
            constraints: Vec::new(),
            default_value: None,
        }
    }

    pub fn alias(mut self, alias: Option<String>) -> Self {
        self.alias = alias;
        self
    }

    pub fn data_type(mut self, data_type: DataType) -> Self {
        self.data_type = data_type;
        self
    }

    pub fn constraints(mut self, constraints: Vec<ColumnConstraint>) -> Self {
        self.constraints = constraints;
        self
    }
}

/// 表
#[derive(Debug, PartialEq, Clone)]
pub(crate) struct TableStmt {
    pub name: String,
    pub alias: Option<String>,
}

/// 创建表语句
#[derive(Debug, Clone)]
pub(crate) struct CreateStmt {
    pub table_name: String,
    pub columns: Vec<ColumnStmt>,
}

/// 删除表语句
#[derive(Debug, Clone)]
pub(crate) struct DropStmt {
    pub table_name: String,
}

/// 显示表列表语句
#[derive(Debug, Clone)]
pub(crate) struct ShowTablesStmt {
    // 不需要特定字段
}

/// 插入表语句
#[derive(Debug, Clone)]
pub(crate) struct InsertStmt {
    pub table_name: String,
    pub columns: Option<Vec<String>>,
    pub values: Vec<Vec<String>>,
}
