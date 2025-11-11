use crate::value::DataType;
use crate::parser::parse_sql_stmt;
use crate::stmt::{ColumnConstraint, Stmt};

#[test]
fn test_parse_select_simple() {
    let sql = "SELECT id, name FROM users;";
    match parse_sql_stmt(sql) {
        Ok((remaining, stmt)) => {
            assert!(remaining.trim().starts_with(';'), "语句应该以分号结束");
            match stmt {
                Stmt::Select(select_stmt) => {
                    assert_eq!(select_stmt.columns.len(), 2, "应该解析到两个列");
                    assert_eq!(select_stmt.columns[0].name, "id".to_string(), "第一列名应该是 id");
                    assert_eq!(select_stmt.columns[1].name, "name".to_string(), "第二列名应该是 name");
                    assert_eq!(select_stmt.table.name, "users", "表名应该是 users");
                }
                _ => panic!("应该解析为 SELECT 语句"),
            }
        }
        Err(e) => panic!("解析失败: {:?}", e),
    }
}

#[test]
fn test_parse_create_table_simple() {
    let sql = "CREATE TABLE test (id INT, name TEXT);";
    match parse_sql_stmt(sql) {
        Ok((remaining, stmt)) => {
            assert!(remaining.trim().starts_with(';'), "语句应该以分号结束");
            match stmt {
                Stmt::Create(create_stmt) => {
                    assert_eq!(create_stmt.table_name, "test", "表名应该是 test");
                    assert_eq!(create_stmt.columns.len(), 2, "应该解析到两个列定义");
                    
                    // 检查第一个列
                    let id_col = &create_stmt.columns[0];
                    assert_eq!(id_col.name, "id", "第一列名应该是 id");
                    assert_eq!(id_col.data_type, DataType::Integer, "第一列类型应该是 Integer");
                    assert!(id_col.constraints.is_empty(), "第一列不应该有约束");
                    
                    // 检查第二个列
                    let name_col = &create_stmt.columns[1];
                    assert_eq!(name_col.name, "name", "第二列名应该是 name");
                    assert_eq!(name_col.data_type, DataType::Text, "第二列类型应该是 Text");
                }
                _ => panic!("应该解析为 CREATE 语句"),
            }
        }
        Err(e) => panic!("解析失败: {:?}", e),
    }
}

#[test]
fn test_parse_create_table_with_constraints() {
    let sql = "CREATE TABLE employees (id INT PRIMARY KEY, name TEXT NOT NULL, age INTEGER);";
    match parse_sql_stmt(sql) {
        Ok((remaining, stmt)) => {
            assert!(remaining.trim().starts_with(';'), "语句应该以分号结束");
            match stmt {
                Stmt::Create(create_stmt) => {
                    assert_eq!(create_stmt.table_name, "employees", "表名应该是 employees");
                    assert_eq!(create_stmt.columns.len(), 3, "应该解析到三个列定义");
                    
                    // 检查第一列（带主键约束）
                    let id_col = &create_stmt.columns[0];
                    assert_eq!(id_col.name, "id", "第一列名应该是 id");
                    assert_eq!(id_col.data_type, DataType::Integer, "第一列类型应该是 Integer");
                    assert_eq!(id_col.constraints.len(), 1, "第一列应该有一个约束");
                    assert_eq!(id_col.constraints[0], ColumnConstraint::PrimaryKey, "约束应该是 PrimaryKey");
                    
                    // 检查第二列（带 NOT NULL 约束）
                    let name_col = &create_stmt.columns[1];
                    assert_eq!(name_col.name, "name", "第二列名应该是 name");
                    assert_eq!(name_col.data_type, DataType::Text, "第二列类型应该是 Text");
                    assert_eq!(name_col.constraints.len(), 1, "第二列应该有一个约束");
                    assert_eq!(name_col.constraints[0], ColumnConstraint::NotNull, "约束应该是 NotNull");
                    
                    // 检查第三列（无约束）
                    let age_col = &create_stmt.columns[2];
                    assert_eq!(age_col.name, "age", "第三列名应该是 age");
                    assert_eq!(age_col.data_type, DataType::Integer, "第三列类型应该是 Integer");
                }
                _ => panic!("应该解析为 CREATE 语句"),
            }
        }
        Err(e) => panic!("解析失败: {:?}", e),
    }
}

#[test]
fn test_parse_create_table_various_types() {
    let sql = "CREATE TABLE mixed_types (id INT, price FLOAT, description VARCHAR, active BOOLEAN);";
    match parse_sql_stmt(sql) {
        Ok((remaining, stmt)) => {
            assert!(remaining.trim().starts_with(';'), "语句应该以分号结束");
            match stmt {
                Stmt::Create(create_stmt) => {
                    assert_eq!(create_stmt.table_name, "mixed_types", "表名应该是 mixed_types");
                    assert_eq!(create_stmt.columns.len(), 4, "应该解析到四个列定义");
                    
                    // 检查不同的数据类型
                    assert_eq!(create_stmt.columns[0].data_type, DataType::Integer, "id 列类型应该是 Integer");
                    assert_eq!(create_stmt.columns[1].data_type, DataType::Float, "price 列类型应该是 Float");
                    assert_eq!(create_stmt.columns[2].data_type, DataType::Text, "description 列类型应该是 Text");
                    assert_eq!(create_stmt.columns[3].data_type, DataType::Boolean, "active 列类型应该是 Boolean");
                }
                _ => panic!("应该解析为 CREATE 语句"),
            }
        }
        Err(e) => panic!("解析失败: {:?}", e),
    }
}

#[test]
fn test_parse_case_insensitive() {
    let sql = "create table users (user_id int primary key, email text);";
    match parse_sql_stmt(sql) {
        Ok((_remaining, stmt)) => {
            match stmt {
                Stmt::Create(create_stmt) => {
                    assert_eq!(create_stmt.table_name, "users", "表名解析应该忽略大小写");
                    assert_eq!(create_stmt.columns[0].data_type, DataType::Integer, "数据类型解析应该忽略大小写");
                }
                _ => panic!("应该解析为 CREATE 语句（忽略大小写）"),
            }
        }
        Err(e) => panic!("解析失败（大小写不敏感测试）: {:?}", e),
    }
}

#[test]
fn test_parse_invalid_sql() {
    let invalid_sql = "INVALID SQL STATEMENT"; 
    match parse_sql_stmt(invalid_sql) {
        Ok(_) => panic!("无效的 SQL 语句应该解析失败"),
        Err(_) => {
            // 预期行为：解析失败
        },
    }
}

#[test]
fn test_parse_invalid_create_table() {
    // 缺少列定义
    let invalid_sql = "CREATE TABLE invalid;";
    match parse_sql_stmt(invalid_sql) {
        Ok(_) => panic!("缺少列定义的 CREATE TABLE 应该解析失败"),
        Err(_) => {
            // 预期行为：解析失败
        },
    }
}

#[test]
fn test_parse_insert_with_columns() {
    let sql = "INSERT INTO users (id, name, age) VALUES ('1', 'Alice', '30');";
    match parse_sql_stmt(sql) {
        Ok((remaining, stmt)) => {
            assert!(remaining.trim().starts_with(';'), "语句应该以分号结束");
            match stmt {
                Stmt::Insert(insert_stmt) => {
                    assert_eq!(insert_stmt.table_name, "users", "表名应该是 users");
                    assert!(insert_stmt.columns.is_some(), "应该有列名定义");
                    if let Some(columns) = &insert_stmt.columns {
                        assert_eq!(columns.len(), 3, "应该有3个列名");
                        assert_eq!(columns[0], "id", "第一列名应该是 id");
                        assert_eq!(columns[1], "name", "第二列名应该是 name");
                        assert_eq!(columns[2], "age", "第三列名应该是 age");
                    }
                    assert_eq!(insert_stmt.values.len(), 1, "应该有1行数据");
                    assert_eq!(insert_stmt.values[0].len(), 3, "每行应该有3个值");
                    assert_eq!(insert_stmt.values[0][0], "1", "第一个值应该是 1");
                    assert_eq!(insert_stmt.values[0][1], "Alice", "第二个值应该是 Alice");
                    assert_eq!(insert_stmt.values[0][2], "30", "第三个值应该是 30");
                },
                _ => panic!("应该解析为 INSERT 语句"),
            }
        },
        Err(e) => panic!("解析失败: {:?}", e),
    }
}

#[test]
fn test_parse_insert_without_columns() {
    let sql = "INSERT INTO products VALUES ('1001', 'Laptop', '999.99');";
    match parse_sql_stmt(sql) {
        Ok((remaining, stmt)) => {
            assert!(remaining.trim().starts_with(';'), "语句应该以分号结束");
            match stmt {
                Stmt::Insert(insert_stmt) => {
                    assert_eq!(insert_stmt.table_name, "products", "表名应该是 products");
                    assert!(insert_stmt.columns.is_none(), "不应该有列名定义");
                    assert_eq!(insert_stmt.values.len(), 1, "应该有1行数据");
                    assert_eq!(insert_stmt.values[0].len(), 3, "每行应该有3个值");
                    assert_eq!(insert_stmt.values[0][0], "1001", "第一个值应该是 1001");
                    assert_eq!(insert_stmt.values[0][1], "Laptop", "第二个值应该是 Laptop");
                    assert_eq!(insert_stmt.values[0][2], "999.99", "第三个值应该是 999.99");
                },
                _ => panic!("应该解析为 INSERT 语句"),
            }
        },
        Err(e) => panic!("解析失败: {:?}", e),
    }
}

#[test]
fn test_parse_insert_multiple_rows() {
    let sql = "INSERT INTO customers (id, name) VALUES ('1', 'Bob'), ('2', 'Charlie');";
    match parse_sql_stmt(sql) {
        Ok((remaining, stmt)) => {
            assert!(remaining.trim().starts_with(';'), "语句应该以分号结束");
            match stmt {
                Stmt::Insert(insert_stmt) => {
                    assert_eq!(insert_stmt.table_name, "customers", "表名应该是 customers");
                    assert!(insert_stmt.columns.is_some(), "应该有列名定义");
                    if let Some(columns) = &insert_stmt.columns {
                        assert_eq!(columns.len(), 2, "应该有2个列名");
                        assert_eq!(columns[0], "id", "第一列名应该是 id");
                        assert_eq!(columns[1], "name", "第二列名应该是 name");
                    }
                    assert_eq!(insert_stmt.values.len(), 2, "应该有2行数据");
                    assert_eq!(insert_stmt.values[0][0], "1", "第一行第一个值应该是 1");
                    assert_eq!(insert_stmt.values[0][1], "Bob", "第一行第二个值应该是 Bob");
                    assert_eq!(insert_stmt.values[1][0], "2", "第二行第一个值应该是 2");
                    assert_eq!(insert_stmt.values[1][1], "Charlie", "第二行第二个值应该是 Charlie");
                },
                _ => panic!("应该解析为 INSERT 语句"),
            }
        },
        Err(e) => panic!("解析失败: {:?}", e),
    }
}

#[test]
fn test_parse_insert_case_insensitive() {
    let sql = "insert into users (user_id, email) values ('1', 'user@example.com');";
    match parse_sql_stmt(sql) {
        Ok((_remaining, stmt)) => {
            match stmt {
                Stmt::Insert(insert_stmt) => {
                    assert_eq!(insert_stmt.table_name, "users", "表名解析应该忽略大小写");
                    assert!(insert_stmt.columns.is_some(), "应该正确解析列名");
                    assert_eq!(insert_stmt.values.len(), 1, "应该解析到一行数据");
                },
                _ => panic!("应该解析为 INSERT 语句（忽略大小写）"),
            }
        },
        Err(e) => panic!("解析失败（大小写不敏感测试）: {:?}", e),
    }
}
