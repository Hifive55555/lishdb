use std::io;

pub type Result<T> = std::result::Result<T, Error>;

/// 数据库错误类型
#[derive(Debug)]
pub enum Error {
    IOError(io::Error),
    AsyncError(tokio::task::JoinError),
    Parser(ParserError),
    Table(TableError),
    Index(IndexError),
    Execution(ExecutionError),
    Storage(StorageError),
    Value(ValueError),
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
            Error::Index(e) => write!(f, "Index Error: {}", e),
            Error::Execution(e) => write!(f, "Execution Error: {}", e),
            Error::Storage(e) => write!(f, "Storage Error: {}", e),
            Error::Value(e) => write!(f, "Value Error: {}", e),
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
impl From<IndexError> for Error {
    fn from(e: IndexError) -> Self {
        Error::Index(e)
    }
}
impl From<ExecutionError> for Error {
    fn from(e: ExecutionError) -> Self {
        Error::Execution(e)
    }
}
impl From<StorageError> for Error {
    fn from(e: StorageError) -> Self {
        Error::Storage(e)
    }
}
impl From<ValueError> for Error {
    fn from(e: ValueError) -> Self {
        Error::Value(e)
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
    /// 表模式不匹配
    SchemaMismatch,
    ColumnExists(String),
    ColumnNotFound(String),
    ColumnDefinitionEmpty(String),
    InsertValuesEmpty(String),
    InsertValuesMismatch(String),
    /// 表ID与表名不一致
    InvalidTableId(String, u64, u64),
}
impl std::error::Error for TableError {}
impl std::fmt::Display for TableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TableError::TableExists(name) => write!(f, "Table '{}' already exists", name),
            TableError::TableNotFound(name) => write!(f, "Table '{}' not found", name),
            TableError::TableNameInvalid(name) => write!(f, "Table name '{}' is invalid", name),
            TableError::SchemaMismatch => write!(f, "Table schema mismatch"),
            TableError::ColumnExists(name) => write!(f, "Column '{}' already exists", name),
            TableError::ColumnNotFound(name) => write!(f, "Column '{}' not found", name),
            TableError::ColumnDefinitionEmpty(name) => write!(f, "Column '{}' definition is empty", name),
            TableError::InsertValuesEmpty(msg) => write!(f, "{}", msg),
            TableError::InsertValuesMismatch(msg) => write!(f, "{}", msg),
            TableError::InvalidTableId(name, id, expected_id) => write!(f, "Table '{}' ID {} does not match expected ID {}", name, id, expected_id),
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
    /// 执行算子时返回了意外的结果类型
    UnexpectedResultType,
    /// 指定的列不存在
    ColumnsNotFound(String),
}
impl std::error::Error for ExecutionError {}
impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionError::UnexpectedResultType => write!(f, "Unexpected result type"),
            ExecutionError::ColumnsNotFound(cols) => write!(f, "Columns not found: {}", cols),
        }
    }
}

/// 存储错误类型
#[derive(Debug)]
pub enum StorageError {
    /// 存储文件不存在
    FileNotFound(String),
    /// 存储文件已存在
    FileExists(String),
    /// 存储文件打开失败
    OpenError(String),
    /// 存储文件读取失败
    ReadError(String),
    /// 存储文件写入失败
    WriteError(String),
    /// 存储文件关闭失败
    CloseError(String),
}
impl std::error::Error for StorageError {}
impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::FileNotFound(path) => write!(f, "Storage file not found: {}", path),
            StorageError::FileExists(path) => write!(f, "Storage file already exists: {}", path),
            StorageError::OpenError(path) => write!(f, "Failed to open storage file: {}", path),
            StorageError::ReadError(path) => write!(f, "Failed to read storage file: {}", path),
            StorageError::WriteError(path) => write!(f, "Failed to write storage file: {}", path),
            StorageError::CloseError(path) => write!(f, "Failed to close storage file: {}", path),
        }
    }
}

/// 值错误类型
#[derive(Debug)]
pub enum ValueError {
    /// 值类型不匹配
    TypeMismatch { expected: String, found: String },
    /// 值格式无效
    InvalidFormat,
    /// 解析失败
    ParseError(String),
    DivisionByZero,
}
impl std::error::Error for ValueError {}
impl std::fmt::Display for ValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueError::TypeMismatch { expected, found } => write!(f, "Value type mismatch: expected {}, found {}", expected, found),
            ValueError::InvalidFormat => write!(f, "Value format is invalid"),
            ValueError::ParseError(msg) => write!(f, "Failed to parse value: {}", msg),
            ValueError::DivisionByZero => write!(f, "Division by zero"),
        }
    }
}

