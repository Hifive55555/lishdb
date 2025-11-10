use crate::cache::CacheManager;
use crate::stmt::{ColumnConstraint, ColumnStmt, CreateStmt, DropStmt, InsertStmt, SelectStmt, ShowTablesStmt, Stmt};
use crate::expression::{Expr, DataType};
use crate::storage::{ColumnMeta, StorageManager, TableMeta};
use crate::catalog::{CatalogManager};
use crate::table::{ColumnAbstract, RowId, TableAbstract};
use crate::error::{ExecutionError, Result, TableError};
use std::sync::Arc;
use std::time::Duration;
use futures_util::future::BoxFuture;
use chrono::{DateTime, Utc};
use log::{debug, warn};

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
    
    /// 插入数据节点 - 向表中插入数据
    Insert(InsertNode),
    
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

/// 插入数据节点
#[derive(Debug)]
struct InsertNode {
    /// 表名
    table_name: String,
    /// 列名列表（可选）
    columns: Option<Vec<String>>,
    /// 值列表（每行一个Vec<String>）
    values: Vec<Vec<String>>,
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
    alias: Vec<Option<String>>, // 投影后的别名
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
    InsertSuccess(String, usize), // 表名和插入的行数
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

/// 为INSERT语句生成执行计划
pub fn generate_insert_plan(stmt: InsertStmt) -> Result<ExecutionPlan> {
    // 检查必要的表名
    if stmt.table_name.is_empty() {
        return Err(TableError::TableNameInvalid("表名不能为空".to_string()).into());
    }
    
    // 检查值列表是否为空
    if stmt.values.is_empty() {
        return Err(TableError::InsertValuesEmpty("插入的值列表不能为空".to_string()).into());
    }
    
    // 创建插入节点
    let insert_node = InsertNode {
        table_name: stmt.table_name.clone(),
        columns: stmt.columns,
        values: stmt.values,
    };
    
    // 创建执行计划
    Ok(ExecutionPlan {
        root: PlanNode::Insert(insert_node),
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
        Stmt::Insert(insert_stmt) => {
            // 为INSERT语句生成执行计划
            generate_insert_plan(insert_stmt)
        },
        // 其他语句类型的执行计划生成
        _ => {
            unimplemented!("目前只支持SELECT、CREATE TABLE、DROP TABLE、SHOW TABLES和INSERT语句")
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
        return Err(TableError::ColumnDefinitionEmpty("列定义不能为空".to_string()).into());
    }
    
    // 3. 构建表定义
    let table_def = TableMeta::new(
        stmt.table_name.clone(),
        stmt.columns.iter().map(|col| ColumnMeta {
            id: 0,  // id未设置
            name: col.name.clone(),
            data_type: col.data_type,
            nullable: col.constraints.contains(&ColumnConstraint::Nullable),
            primary_key: col.constraints.contains(&ColumnConstraint::PrimaryKey),
            default_value: col.default_value.clone(),
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
    
    // // 3. 如果有WHERE子句，添加过滤节点
    // if let Some(condition) = stmt.where_expr {
    //     plan = PlanNode::Filter(FilterNode {
    //         child: Box::new(plan),
    //         condition,
    //     });
    // }
    
    // 4. 添加投影节点，选择指定的列
    // 处理ColumnWithAlias中的列名和别名
    let columns = stmt.columns.iter()
        .map(|col| col.name.clone())
        .collect();
    let alias = stmt.columns.iter()
        .map(|col| col.alias.clone())
        .collect();
    
    plan = PlanNode::Projection(ProjectionNode {
        child: Box::new(plan),
        columns,
        alias,
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
                    self_ref.execute_projection(table, projection_node).await
                },
                PlanNode::Insert(insert_node) => {
                    // 执行插入操作
                    self_ref.execute_insert(insert_node).await
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

        // 处理星号（*）通配符
        let requested_columns: Vec<String> = if scan_node.column_names.contains(&"*".to_string()) {
            // 如果包含星号，使用表的所有列
            table_meta.columns.iter().map(|col| col.name.clone()).collect()
        } else {
            // 否则，确保请求的列都存在
            scan_node.column_names.iter()
                .filter(|col_name| table_meta.columns.iter().any(|col| col.name == **col_name))
                .cloned()
                .collect()
        };

        // 如果请求的列都不存在，返回错误
        if requested_columns.is_empty() {
            warn!("No valid columns requested for scan operation on table '{}'", scan_node.table_name);
            return Err(ExecutionError::ColumnsNotFound(requested_columns.join(", ")).into());
        }

        // 构建抽象表结构，只包含请求的列
        let columns = table_meta.columns.iter()
            .filter(|col| requested_columns.contains(&col.name))
            .map(|col| ColumnAbstract {
                id: col.id,
                name: col.name.clone(),
                table_name: scan_node.table_name.clone(),
                data_type: col.data_type.clone(),
            })
            .collect();

        // 获取表的行ID列表
        let rows = RowId::get_vec(table_meta.id, table_meta.row_count);
        
        let table = TableAbstract {
            columns,
            rows,
        };
        
        Ok(ExecutionResult::Table(table))
    }
    
    /// 执行过滤操作
    async fn execute_filter(&self, table: TableAbstract, condition: &Expr) -> Result<ExecutionResult> {
        // 过滤需要做的操作：
        // 1. 遍历所有行
        // 2. 对每一行应用表达式
        // 3. 如果表达式为真，保留该行；否则，过滤掉
        
        // 对于简单实现，我们只支持基本的相等条件过滤
        // 获取表名和列信息，用于值查找
        let table_name = &table.columns[0].table_name; // 假设所有列来自同一表
        
        // 过滤行
        let filtered_rows: Vec<RowId> = table.rows.into_iter()
            .filter(|row_id| {
                // 对于简单实现，我们假设condition是一个标识符表达式（列名）与常量的比较
                // 这里简化处理，实际应该实现完整的表达式求值
                
                // 检查表达式类型
                // 在真实场景中，这里应该根据表达式结构进行递归求值
                // 现在我们只是简单地将所有行都保留，因为完整的表达式求值比较复杂
                
                // TODO: 实现完整的表达式求值逻辑
                // 对于演示，我们目前保留所有行
                true
            })
            .collect();
        
        // 创建过滤后的表抽象
        let filtered_table = TableAbstract {
            columns: table.columns,
            rows: filtered_rows,
        };
        
        Ok(ExecutionResult::Table(filtered_table))
    }
    
    /// 执行投影操作
    async fn execute_projection(&self, table: TableAbstract, projection_node: &ProjectionNode) -> Result<ExecutionResult> {
        // 处理通配符（*）
        let columns = if projection_node.columns.contains(&"*".to_string()) {
            // 如果包含星号，使用表的所有列
            table.columns.iter().map(|col| col.name.clone()).collect()
        } else {
            projection_node.columns.clone()
        };

        // 使用TableAbstract的project方法
        let mut projected_table = table.project(&columns)?;

        // 使用别名
        let aliased_columns = projection_node.alias.iter().zip(&columns)
            .map(|(alias, col)| alias.as_ref().map_or(col.clone(), |a| a.clone()))
            .collect::<Vec<_>>();
        projected_table.columns.iter_mut().zip(aliased_columns).for_each(|(col, alias)| col.name = alias);

        Ok(ExecutionResult::Table(projected_table))
    }
    
    /// 执行插入操作
    async fn execute_insert(&self, insert_node: &InsertNode) -> Result<ExecutionResult> {
        // 1. 检查表是否存在
        let table_meta = match self.catalog.get_table_meta(&insert_node.table_name).await? {
            Some(table) => table,
            None => return Err(TableError::TableNotFound(insert_node.table_name.clone()).into()),
        };
        
        // 2. 确定要插入的列
        let target_columns = if let Some(columns) =&insert_node.columns {
            // 检查指定的列是否都存在于表中
            for col_name in columns {
                if !table_meta.columns.iter().any(|col| col.name == *col_name) {
                    return Err(TableError::ColumnNotFound(format!("列 '{}' 不存在于表 '{}' 中", col_name, insert_node.table_name)).into());
                }
            }
            columns.clone()
        } else {
            // 如果没有指定列，使用表的所有列
            table_meta.columns.iter().map(|col| col.name.clone()).collect()
        };
        
        // 3. 验证每行的值数量是否与列数量匹配
        for (row_idx, values) in insert_node.values.iter().enumerate() {
            if values.len() != target_columns.len() {
                return Err(TableError::InsertValuesMismatch(format!("第 {} 行的值数量 ({}) 与列数量 ({}) 不匹配", 
                    row_idx + 1, values.len(), target_columns.len())).into());
            }
        }
        
        // 4. 执行数据插入
        self.storage.insert_data(&table_meta, &target_columns, &insert_node.values)?;
        
        // 5. 更新表的行数
        let rows_inserted = insert_node.values.len();
        let mut updated_meta = table_meta.clone();
        updated_meta.row_count += rows_inserted as u64;
        
        // 6. 更新目录中的表元数据
        self.catalog.update_table_meta(updated_meta).await?;
        
        // 7. 更新缓存（如果需要）
        // 这里可以添加缓存更新逻辑
        
        // 8. 返回成功结果
        Ok(ExecutionResult::InsertSuccess(insert_node.table_name.clone(), rows_inserted))
    }
}

/// 将查询结果从抽象表转换为实际数据值
pub async fn get_query_results(
    result: ExecutionResult,
    cache_manager: Arc<CacheManager>
) -> Result<crate::table::TableActual> {
    match result {
        ExecutionResult::Table(table_abstract) => {
            // 从抽象表转换为实际表，这会从缓存中获取行数据
            // 如果缓存中没有，会触发从存储中加载
            table_abstract.to_actual(cache_manager)
        },
        _ => Err(ExecutionError::UnexpectedResultType.into())
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
