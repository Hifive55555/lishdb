use crate::cache::CacheManager;
use crate::stmt::{ColumnConstraint, ColumnStmt, CreateStmt, DropStmt, SelectStmt, ShowTablesStmt, Stmt};
use crate::expression::{Expr, DataType};
use crate::storage::{ColumnMeta, StorageManager, TableMeta};
use crate::catalog::{CatalogManager};
use crate::table::{ColumnAbstract, RowId, TableAbstract};
use crate::error::{ColumnError, Error, ExecutionError, Result, TableError};
use std::sync::Arc;
use std::time::Duration;
use futures_util::future::BoxFuture;
use chrono::{DateTime, Utc};

/// 执行计划节点类型
#[derive(Debug)]
enum PlanNode {
    /// 新建表节点 - 创建一个新表
    CreateTable(CreateTableNode),
    
    /// 删除表节点 - 删除一个表
    DropTable(DropTableNode),
    
    /// 显示表列表节点
    ShowTables(ShowTablesNode),

    /// 表扫描节点 - 从表中读取所有行
    Scan(ScanNode),
    
    /// 过滤节点 - 根据条件筛选行
    Filter(FilterNode),
    
    /// 投影节点 - 选择指定的列
    Projection(ProjectionNode),
    
    // 可以根据需要添加更多节点类型
    // Join, Sort, Aggregate 等
}

/// 新建表节点 - 创建一个新表
#[derive(Debug)]
struct CreateTableNode {
    /// 新表的定义
    table_def: TableMeta,
}

/// 显示表列表节点
#[derive(Debug)]
struct ShowTablesNode;

/// 删除表节点 - 删除一个表
#[derive(Debug)]
struct DropTableNode {
    /// 要删除的表名
    table_name: String,
}

/// 扫描节点
#[derive(Debug)]
struct ScanNode {
    /// 需要扫描的表名
    table_name: String,
    /// 需要扫描的列名
    column_names: Vec<String>,
}

/// 过滤节点
#[derive(Debug)]
struct FilterNode {
    child: Box<PlanNode>,
    condition: Expr,
}

/// 投影节点
#[derive(Debug)]
struct ProjectionNode {
    child: Box<PlanNode>,
    columns: Vec<String>, // 存储要投影的列名
}

/// 执行计划
#[derive(Debug)]
pub struct ExecutionPlan {
    root: PlanNode,
}

/// 执行结果类型
pub enum ExecutionResult {
    Table(TableAbstract),
    CreateTableSuccess(String),
    DropTableSuccess(String),
    ShowTablesSuccess(Vec<String>),
    // 可以添加其他类型的执行结果
}

/// 为SHOW TABLES语句生成执行计划
pub fn generate_show_tables_plan(_stmt: crate::stmt::ShowTablesStmt) -> Result<ExecutionPlan> {
    // 直接创建执行计划，不需要额外参数
    Ok(ExecutionPlan {
        root: PlanNode::ShowTables(ShowTablesNode {}),
    })
}

/// 为DROP TABLE语句生成执行计划
pub fn generate_drop_plan(stmt: DropStmt) -> Result<ExecutionPlan> {
    // 检查必要的表名
    if stmt.table_name.is_empty() {
        return Err(TableError::TableNameInvalid("表名不能为空".to_string()).into());
    }
    
    // 创建删除表节点
    let drop_table_node = DropTableNode {
        table_name: stmt.table_name.clone(),
    };
    
    // 创建执行计划
    Ok(ExecutionPlan {
        root: PlanNode::DropTable(drop_table_node),
    })
}

/// 生成执行计划
pub fn generate_execution_plan(stmt: Stmt) -> Result<ExecutionPlan> {
    match stmt {
        Stmt::Select(select_stmt) => {
            // 为SELECT语句生成执行计划
            generate_select_plan(select_stmt)
        },
        Stmt::Create(create_stmt) => {
            // 为CREATE TABLE语句生成执行计划
            generate_create_plan(create_stmt)
        },
        Stmt::Drop(drop_stmt) => {
            // 为DROP TABLE语句生成执行计划
            generate_drop_plan(drop_stmt)
        },
        Stmt::ShowTables(show_tables_stmt) => {
            // 为SHOW TABLES语句生成执行计划
            generate_show_tables_plan(show_tables_stmt)
        },
        // 其他语句类型的执行计划生成
        _ => {
            unimplemented!("目前只支持SELECT、CREATE TABLE、DROP TABLE和SHOW TABLES语句")
        },
    }
}

/// 为CREATE TABLE语句生成执行计划
pub fn generate_create_plan(stmt: CreateStmt) -> Result<ExecutionPlan> {
    // 1. 检查必要的表名
    if stmt.table_name.is_empty() {
        return Err(TableError::TableNameInvalid("表名不能为空".to_string()).into());
    }
    
    // 2. 检查列定义是否为空
    if stmt.columns.is_empty() {
        return Err(ColumnError::ColumnDefinitionEmpty("列定义不能为空".to_string()).into());
    }
    
    // 3. 构建表定义
    let table_def = TableMeta::new(
        stmt.table_name.clone(),
        stmt.columns.iter().map(|col| ColumnMeta {
            name: col.name.clone(),
            data_type: col.data_type,
            nullable: col.constraints.contains(&ColumnConstraint::Nullable),
            primary_key: col.constraints.contains(&ColumnConstraint::PrimaryKey),
            default_value: col.default_value.clone(),
            ..Default::default()
        },).collect(),
    );
    
    // 4. 创建新建表节点
    let create_table_node = CreateTableNode {
        table_def,
    };
    
    // 5. 创建执行计划
    Ok(ExecutionPlan {
        root: PlanNode::CreateTable(create_table_node),
    })
}

/// 为SELECT语句生成执行计划
pub fn generate_select_plan(stmt: SelectStmt) -> Result<ExecutionPlan> {
    // 1. 检查必要的表名
    if stmt.table.name.is_empty() {
        return Err(TableError::TableNameInvalid("表名不能为空".to_string()).into());
    }
    
    // 2. 从最底层开始构建：扫描表
    let mut plan = PlanNode::Scan(ScanNode {
        table_name: stmt.table.name.clone(),
        column_names: stmt.columns.iter().map(|col| col.name.clone()).collect(),
    });
    
    // 3. 如果有WHERE子句，添加过滤节点
    if let Some(condition) = stmt.where_expr {
        plan = PlanNode::Filter(FilterNode {
            child: Box::new(plan),
            condition,
        });
    }
    
    // 4. 添加投影节点，选择指定的列
    // 处理ColumnWithAlias中的列名和别名
    let columns = stmt.columns.iter()
        .map(|col| {
            // 优先使用别名，如果有的话
            if let Some(alias) = &col.alias {
                alias.clone()
            } else {
                // 否则使用原始列名
                col.name.clone()
            }
        })
        .collect();
    
    plan = PlanNode::Projection(ProjectionNode {
        child: Box::new(plan),
        columns,
    });
    
    // 5. 创建执行计划
    Ok(ExecutionPlan {
        root: plan,
    })
}



/// 执行计划执行器
pub struct Executor {
    storage: Arc<StorageManager>,  // 存储管理器
    cache_manager: Arc<CacheManager>,  // 缓存管理器
    catalog: Arc<CatalogManager>,  // 目录管理器
}

impl Executor {
    /// 创建新的执行器
    pub fn new(storage: Arc<StorageManager>, catalog: Arc<CatalogManager>, cache_manager: Arc<CacheManager>) -> Self {
        Self {
            storage,
            catalog,
            cache_manager,
        }
    }
    
    /// 执行执行计划
    pub async fn execute(&self, plan: ExecutionPlan) -> Result<ExecutionResult> {
        self.execute_node(&plan.root).await
    }
    
    /// 递归执行计划节点
    fn execute_node<'a>(&'a self, node: &'a PlanNode) -> BoxFuture<'a, Result<ExecutionResult>> {
        let self_ref = self;
        let node_ref = node;
        Box::pin(async move {
            match node_ref {
                PlanNode::CreateTable(create_table_node) => {
                    // 执行新建表操作
                    self_ref.execute_create_table(create_table_node).await
                },
                PlanNode::DropTable(drop_table_node) => {
                    // 执行删除表操作
                    self_ref.execute_drop_table(drop_table_node).await
                },
                PlanNode::ShowTables(_show_tables_node) => {
                    // 执行显示表列表操作
                    self_ref.execute_show_tables().await
                },
                PlanNode::Scan(scan_node) => {
                    // 扫描表并返回所有行
                    self_ref.execute_scan(scan_node).await
                },
                PlanNode::Filter(filter_node) => {
                    // 先执行子节点，然后根据条件过滤结果
                    let ExecutionResult::Table(table) = self_ref.execute_node(&*filter_node.child).await? else {
                        return Err(ExecutionError::UnexpectedResultType.into());
                    };
                    self_ref.execute_filter(table, &filter_node.condition).await
                },
                PlanNode::Projection(projection_node) => {
                    // 先执行子节点，然后投影指定的列
                    let ExecutionResult::Table(table) = self_ref.execute_node(&*projection_node.child).await? else {
                        return Err(ExecutionError::UnexpectedResultType.into());
                    };
                    self_ref.execute_projection(table, &projection_node.columns).await
                },
            }
        })
    }
    
    /// 执行显示表列表操作
    async fn execute_show_tables(&self) -> Result<ExecutionResult> {
        // 从catalog获取所有表名
        let table_names = self.catalog.list_tables().await?;
        
        Ok(ExecutionResult::ShowTablesSuccess(table_names))
    }
    
    /// 执行删除表操作
    async fn execute_drop_table(&self, drop_table_node: &DropTableNode) -> Result<ExecutionResult> {
        self.catalog.drop_table(&drop_table_node.table_name).await?;
        
        Ok(ExecutionResult::DropTableSuccess(drop_table_node.table_name.clone()))
    }

    /// 执行新建表操作
    async fn execute_create_table(&self, create_table_node: &CreateTableNode) -> Result<ExecutionResult> {
        self.catalog.create_table(create_table_node.table_def.clone()).await?;
        
        Ok(ExecutionResult::CreateTableSuccess(create_table_node.table_def.name.clone()))
    }
        
    /// 执行扫描操作
    async fn execute_scan(&self, scan_node: &ScanNode) -> Result<ExecutionResult> {
        // 检查表是否存在
        let table_meta = match self.catalog.get_table_meta(&scan_node.table_name).await? {
            Some(table) => table,
            None => return Err(TableError::TableNotFound(scan_node.table_name.clone()).into()),
        };

        // 构建一个抽象表
        let columns = table_meta.columns.iter().map(|col| ColumnAbstract {
            id: col.id,
            name: col.name.clone(),
            table_name: scan_node.table_name.clone(),
            data_type: col.data_type.clone(),
        }).collect();

        let rows = RowId::get_vec(table_meta.id, table_meta.row_count);
        
        let table = TableAbstract {
            columns,
            rows,
        };
        
        Ok(ExecutionResult::Table(table))
    }
    
    /// 执行过滤操作
    async fn execute_filter(&self, table: TableAbstract, _condition: &Expr) -> Result<ExecutionResult> {
        // 过滤需要做的操作：
        // 1. 遍历所有行
        // 2. 对每一行应用表达式
        // 3. 如果表达式为真，保留该行；否则，过滤掉

        Ok(ExecutionResult::Table(table))
    }
    
    /// 执行投影操作
    async fn execute_projection(&self, table: TableAbstract, columns: &[String]) -> Result<ExecutionResult> {
        unimplemented!("projection operator is not implemented")
    }
}

/// 执行执行计划
pub async fn execute_plan(
    plan: ExecutionPlan, 
    storage: Arc<StorageManager>, 
    catalog: Arc<CatalogManager>,
    cache_manager: Arc<CacheManager>
) -> Result<ExecutionResult> {
    let executor = Executor::new(storage, catalog, cache_manager);
    executor.execute(plan).await
}
