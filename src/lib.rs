mod parser;
mod expression;
mod optimizer;
mod executor;
mod storage;
mod cache;
mod catalog;
mod index;
mod stmt;
mod table;
pub mod error;
#[cfg(test)]
mod tests {
    #[cfg(test)]
    mod test_parser;
}

use tokio::sync::Semaphore;
use std::sync::Arc;
use error::{Error, Result};
use log::{info, warn, trace};

pub use parser::parse_sql;
pub use optimizer::optimize_stmt;
pub use storage::{StorageManager, StorageConfig};
pub use catalog::CatalogManager;
pub use index::{IndexManager, IndexDef, IndexType};
pub use stmt::Stmt;
pub use expression::DataType;
pub use table::TableActual;

use executor::ExecutionResult;
use cache::CacheManager;

pub enum HandleResult {
    Table(TableActual),
    Message(String),
}

pub struct DbHandler {
    semaphore: Arc<Semaphore>,
    storage: Arc<StorageManager>,
    catalog: Arc<CatalogManager>,
    cache: Arc<CacheManager>,
}

impl Default for DbHandler {
    fn default() -> Self {
        DbHandler {
            semaphore: Arc::new(Semaphore::new(1)),
            storage: Arc::new(StorageManager::default()),
            catalog: Arc::new(CatalogManager::default()),
            cache: Arc::new(CacheManager::default()),
        }
    }
}

impl DbHandler {
    pub fn new(concurrency: usize, storage: Arc<StorageManager>, catalog: Arc<CatalogManager>) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(concurrency)),
            storage,
            catalog,
            cache: Arc::new(CacheManager::default()),
        }
    }

    pub async fn handle(&self, sql: &str) -> Result<HandleResult> {
        let sem = self.semaphore.clone();
        let sql = sql.to_string();
        let storage = self.storage.clone();
        let catalog = self.catalog.clone();
        let cache = self.cache.clone();

        // 异步执行 SQL 语句解析、优化和执行
        let handle: tokio::task::JoinHandle<Result<HandleResult>> = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            // 1. 解析 SQL 语句
            let stmt = parse_sql(&sql)?;
            trace!("Parsed SQL Statement: {:?}", stmt);
            
            // 2. 优化 SQL 语句
            let optimized_stmt = optimize_stmt(stmt);
            trace!("Optimized SQL Statement: {:?}", optimized_stmt);
            
            // 3. 生成执行计划
            let plan = executor::generate_execution_plan(optimized_stmt)?;
            trace!("Execution Plan: {:?}", plan);
            
            // 4. 执行查询
            let result = executor::execute_plan(plan, storage.clone(), catalog.clone(), cache.clone()).await?;

            // 读取实际数据
            let result = match result {
                ExecutionResult::Table(table) => HandleResult::Table(table.to_actual(cache.clone())),
                ExecutionResult::CreateTableSuccess(table_name) => HandleResult::Message(format!("创建表 {table_name} 成功！")),
                ExecutionResult::DropTableSuccess(table_name) => HandleResult::Message(format!("删除表 {table_name} 成功！")),
                ExecutionResult::ShowTablesSuccess(table_names) => {
                    if table_names.is_empty() {
                        HandleResult::Message("数据库中暂无表".to_string())
                    } else {
                        let tables_str = table_names.join(", ");
                        HandleResult::Message(format!("所有表: {}", tables_str))
                    }
                },
            };
            
            Ok(result)
        });
        
        // 等待任务完成并返回结果
        let result = handle.await??;
        Ok(result)
    }
}