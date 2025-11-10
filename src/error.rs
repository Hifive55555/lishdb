use std::io;

pub type Result<T> = std::result::Result<T, Error>;

/// 数据库错误类型
#[derive(Debug)]
pub enum Error {
    IOError(io::Error),
    AsyncError(tokio::task::JoinError),
    Parser(ParserError),
    Table(TableError),
    Column(ColumnError),
    Index(IndexError),
    ExecutionError(ExecutionError),
    Internal(String),
}

impl std::error::Error for Error {}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Parser(e) => write!(f, "Parser Error: {}", e),
            Error::Table(e) => write!(f, "Table Error: {}", e),
            Error::IOError(e) => write!(f, "IO Error: {}", e),
            Error::AsyncError(e) => write!(f, "Async Error: {}", e),
            Error::Column(e) => write!(f, "Column Error: {}", e),
            Error::Index(e) => write!(f, "Index Error: {}", e),
            Error::ExecutionError(e) => write!(f, "Execution Error: {}", e),
            Error::Internal(e) => write!(f, "Internal Error: {}", e),
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IOError(e)
    }
}
impl From<tokio::task::JoinError> for Error {
    fn from(e: tokio::task::JoinError) -> Self {
        Error::AsyncError(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::IOError(e.into())
    }
}
impl From<ParserError> for Error {
    fn from(e: ParserError) -> Self {
        Error::Parser(e)
    }
}
impl From<TableError> for Error {
    fn from(e: TableError) -> Self {
        Error::Table(e)
    }
}
impl From<ColumnError> for Error {
    fn from(e: ColumnError) -> Self {
        Error::Column(e)
    }
}
impl From<IndexError> for Error {
    fn from(e: IndexError) -> Self {
        Error::Index(e)
    }
}
impl From<ExecutionError> for Error {
    fn from(e: ExecutionError) -> Self {
        Error::ExecutionError(e)
    }
}


/// 解析器错误类型
#[derive(Debug)]
pub enum ParserError {
    InvalidSyntax,
}

impl std::error::Error for ParserError {}
impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserError::InvalidSyntax => write!(f, "Invalid SQL syntax"),
        }
    }
}

/// 表错误类型
#[derive(Debug)]
pub enum TableError {
    TableExists(String),
    TableNotFound(String),
    TableNameInvalid(String),
}
impl std::error::Error for TableError {}
impl std::fmt::Display for TableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TableError::TableExists(name) => write!(f, "Table '{}' already exists", name),
            TableError::TableNotFound(name) => write!(f, "Table '{}' not found", name),
            TableError::TableNameInvalid(name) => write!(f, "Table name '{}' is invalid", name),
        }
    }
}

/// 列错误类型
#[derive(Debug)]
pub enum ColumnError {
    ColumnExists(String),
    ColumnNotFound(String),
    ColumnDefinitionEmpty(String),
}
impl std::error::Error for ColumnError {}
impl std::fmt::Display for ColumnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColumnError::ColumnExists(name) => write!(f, "Column '{}' already exists", name),
            ColumnError::ColumnNotFound(name) => write!(f, "Column '{}' not found", name),
            ColumnError::ColumnDefinitionEmpty(name) => write!(f, "Column '{}' definition is empty", name),
        }
    }
}

/// 索引错误类型
#[derive(Debug)]
pub enum IndexError {
    IndexExists(String),
    IndexNotFound(String),
    IndexUniqueViolation(String),
}
impl std::error::Error for IndexError {}
impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexError::IndexExists(name) => write!(f, "Index '{}' already exists", name),
            IndexError::IndexNotFound(name) => write!(f, "Index '{}' not found", name),
            IndexError::IndexUniqueViolation(msg) => write!(f, "Index unique violation: {}", msg),
        }
    }
}

/// 执行错误类型
#[derive(Debug)]
pub enum ExecutionError {
    UnexpectedResultType,
}
impl std::error::Error for ExecutionError {}
impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionError::UnexpectedResultType => write!(f, "Unexpected result type"),
        }
    }
}
