use nom::branch::alt;
use nom::bytes::complete::{tag, tag_no_case, take_till1};
use nom::character::complete::{alpha1, alphanumeric1, multispace0, multispace1};
use nom::combinator::{map, opt, recognize, value};
use nom::multi::{many0, separated_list1};
use nom::sequence::{delimited, pair, preceded, terminated};
use nom::{IResult, Parser};
use log::{debug, error, info, trace, warn};

use crate::expression::*;
use crate::error::Result;
use crate::stmt::*;
use crate::value::DataType;

/// 解析标识符（支持字母、数字、下划线，必须以字母或下划线开头）
fn identifier(input: &str) -> IResult<&str, &str> {
    // 首字符：字母（a-z, A-Z）或下划线（_）
    let first_char = alt((alpha1, tag("_")));
    // 后续字符：字母、数字或下划线
    let rest_chars = many0(alt((alpha1, tag("_"), nom::character::complete::digit1)));
    
    // 组合：首字符 + 后续字符，返回完整标识符
    recognize(pair(first_char, rest_chars)).parse(input)
}

mod parse_expr {
    use super::*;

    fn parse_constant_expr(input: &str) -> IResult<&str, ConstantExpr> {
        // 带引号的字符串常量
        let parse_quoted = map(
            delimited(tag("'"), take_till1(|c| c == '\''), tag("'")),
            |s: &str| ConstantExpr { value: s.to_string(), data_type: DataType::Text },
        );

        // 浮点数 (e.g. 12.34)
        let parse_float = map(
            recognize(pair(
                nom::character::complete::digit1,
                pair(tag("."), nom::character::complete::digit1),
            )),
            |s: &str| ConstantExpr { value: s.to_string(), data_type: DataType::Float },
        );

        // 整数
        let parse_int = map(
            nom::character::complete::digit1,
            |s: &str| ConstantExpr { value: s.to_string(), data_type: DataType::Integer },
        );

        // 布尔值 TRUE / FALSE
        let parse_bool = map(
            alt((tag_no_case("TRUE"), tag_no_case("FALSE"))),
            |s: &str| ConstantExpr { value: s.to_uppercase(), data_type: DataType::Boolean },
        );

        alt((parse_quoted, parse_float, parse_int, parse_bool)).parse(input)
    }

    /// 解析标识符表达式（列名或别名）
    fn parse_identifier_expr(input: &str) -> IResult<&str, IdentifierExpr> {
        let (input, name) = identifier(input)?;
        Ok((input, IdentifierExpr { name: name.to_string() }))
    }

    fn parse_function_expr(input: &str) -> IResult<&str, FunctionExpr> {
        // 匹配函数名
        let (input, function_name) = alt((
            tag_no_case("LENGTH"),
        )).parse(input)?;

        // 匹配参数列表
        let (input, args) = delimited(
            tag("("),
            separated_list1(tag(","), parse),
            tag(")"),
        ).parse(input)?;

        // 根据函数名创建函数表达式
        let function_type = match function_name {
            "LENGTH" => FunctionType::LENGTH,
            _ => {
                warn!("解析到未知函数名：{}", function_name);
                return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)));
            }
        };

        Ok((input, FunctionExpr { function_type, args }))
    }

    // 解析括号包裹的表达式（如 "(1+2)" -> 内部表达式）
    fn parse_parenthesized(input: &str) -> IResult<&str, Expr> {
        delimited(
            tag("("),    // 左括号
            parse,      // 递归解析括号内的表达式（核心！）
            tag(")"),    // 右括号
        ).parse(input)
    }

    // 解析原子（数字或括号表达式，优先级最高）
    fn parse_factor(input: &str) -> IResult<&str, Expr> {
        alt((
            parse_parenthesized,  // 括号表达式
            parse_constant_expr.map(|expr| Expr::new(Box::new(expr))),  // 常量表达式
            parse_identifier_expr.map(|expr| Expr::new(Box::new(expr))), // 标识符表达式
        )).parse(input)
    }

    // 解析乘除运算（优先级次之）
    fn parse_term(input: &str) -> IResult<&str, Expr> {
        // 先解析第一个因子
        let (mut input, mut acc) = parse_factor(input)?;
        
        // 处理连续的乘除运算
        while let Ok((remaining_input, (op, factor))) = pair(
            preceded(multispace0, alt((tag("*"), tag("/")))),
            preceded(multispace0, parse_factor),
        ).parse(input) {
            let op = match op {
                "*" => BinaryOp::Multiply,
                "/" => BinaryOp::Divide,
                _ => unreachable!(),
            };
            acc = Expr::new(Box::new(BinaryExpr {
                left: acc,
                op,
                right: factor,
            }));
            input = remaining_input;
        }
        
        Ok((input, acc))
    }

    fn parse_binary_expr(input: &str) -> IResult<&str, Expr> {
        // 解析加减运算（优先级最低，顶层解析器）
        // 用fold_many0处理连续的加减运算（如 1+2-3）
        let (mut input, mut acc) = parse_term(input)?;

        // 处理连续的加减运算
        while let Ok((remaining_input, (op, term))) = pair(
            preceded(multispace0, alt((tag("+"), tag("-")))),
            preceded(multispace0, parse_term),
        ).parse(input) {
            let op = match op {
                "+" => BinaryOp::Add,
                "-" => BinaryOp::Subtract,
                _ => unreachable!(),
            };
            acc = Expr::new(Box::new(BinaryExpr {
                left: acc,
                op,
                right: term,
            }));
            input = remaining_input;
        }
        
        Ok((input, acc))
    }

    pub(super) fn parse(input: &str) -> IResult<&str, Expr> {
        trace!("解析表达式：{:?}", input);
        
        alt((
            parse_function_expr.map(|expr| Expr::new(Box::new(expr))),
            parse_binary_expr,
            parse_constant_expr.map(|expr| Expr::new(Box::new(expr))),
            parse_identifier_expr.map(|expr| Expr::new(Box::new(expr)))
        )).parse(input)
    }
}

mod parse_create {
    use super::*;
    use crate::stmt::ColumnConstraint;

    pub(super) fn parse(input: &str) -> IResult<&str, CreateStmt> {
        let (input, _) = multispace0(input)?;
        trace!("开始解析CREATE TABLE语句：{:?}", input);

        // 解析表名
        let (input, table_name) = identifier(input)?;
        let (input, _) = multispace0(input)?;
        trace!("解析到表名：{:?}", table_name);

        // 解析列定义列表
        let (input, columns) = delimited(
            tag("("),
            separated_list1(
                delimited(multispace0, tag(","), multispace0),
                parse_column_def
            ),
            tag(")")
        ).parse(input)?;
        trace!("解析到列定义：{:?}", columns);

        Ok((input, CreateStmt {
            table_name: table_name.to_string(),
            columns
        }))
    }

    // 解析单个列定义
    fn parse_column_def(input: &str) -> IResult<&str, ColumnStmt> {
        let (input, _) = multispace0(input)?;
        
        // 列名
        let (input, column_name) = identifier(input)?;
        let (input, _) = multispace1(input)?;
        
        // 数据类型
        let (input, data_type) = parse_data_type(input)?;
        let (input, _) = multispace0(input)?;
        
        // 可选约束
        let mut constraints = Vec::new();
        let mut remaining_input = input;
        
        while let Ok((next_input, constraint)) = parse_column_constraint(remaining_input) {
            constraints.push(constraint);
            remaining_input = next_input;
            let (next, _) = multispace0(next_input)?;
            remaining_input = next;
        }
        
        Ok((remaining_input, ColumnStmt {
            name: column_name.to_string(),
            alias: None,
            data_type,
            constraints,
            default_value: None,
        }))
    }

    // 解析数据类型
    fn parse_data_type(input: &str) -> IResult<&str, DataType> {
        let (input, data_type_str) = alt((
            tag_no_case("INTEGER"),
            tag_no_case("INT"),
            tag_no_case("FLOAT"),
            tag_no_case("TEXT"),
            tag_no_case("VARCHAR"),
            tag_no_case("BOOLEAN"),
        )).parse(input)?;
        
        let data_type = match data_type_str.to_uppercase().as_str() {
            "INT" | "INTEGER" => DataType::Integer,
            "FLOAT" => DataType::Float,
            "TEXT" | "VARCHAR" => DataType::Text,
            "BOOLEAN" => DataType::Boolean,
            _ => {
                warn!("未知数据类型：{}", data_type_str);
                return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)));
            }
        };
        
        Ok((input, data_type))
    }

    // 解析列约束
    fn parse_column_constraint(input: &str) -> IResult<&str, ColumnConstraint> {
        let (input, constraint) = alt((
            value(ColumnConstraint::PrimaryKey, tag_no_case("PRIMARY KEY")),
            value(ColumnConstraint::NotNull, (tag_no_case("NOT"), multispace0, tag_no_case("NULL"))),
            value(ColumnConstraint::Nullable, tag_no_case("NULL")),
        )).parse(input)?;
        Ok((input, constraint))
    }
}

mod parse_select {
    use nom::bytes::complete::take_until;

    use super::*;

    pub(super) fn parse(input: &str) -> IResult<&str, SelectStmt> {
        let (input, _) = multispace0(input)?;
        trace!("开始解析SELECT语句：{:?}", input);

        // 解析列名
        let (input, columns) = alt((
            take_until("FROM"),
            take_until("from"),
        )).parse(input)?;
        let (_, columns) = parse_column_list(columns)?;
        trace!("解析到列名：{:?}", columns);

        // 解析表名
        let (input, table_name) = parse_table_name(input)?;
        trace!("解析到表名：{:?}", table_name);

        // 解析WHERE子句
        let (input, where_expr) = opt(parse_where_clause).parse(input)?;
        trace!("解析到WHERE子句：{:?}", where_expr);
        
        Ok((input, SelectStmt {
            columns,
            table: table_name,
            where_expr,
        }))
    }

    fn parse_column_list(input: &str) -> IResult<&str, Vec<ColumnStmt>> {
        let (input, _) = multispace0(input)?;
        // 拿到列名和别名（去除前后空格）
        let cols = input.trim().split(',').map(|s| s.trim()).collect::<Vec<_>>();

        // 解析是否有别名
        let mut cols_with_alias = Vec::new();

        for col_str in cols {
            // 尝试匹配列名
            let (other, col_name) = alt((
                identifier,
                tag("*"),
            )).parse(col_str)?;

            // 尝试匹配别名
            let (_other, alias) = opt(
                // 以 AS 或空格开头
                preceded(
                    alt((
                        delimited(multispace1, tag_no_case("AS"), multispace1),  // AS 关键字
                        multispace1,  // 空格
                    )),
                    identifier  // 别名
                )
            ).parse(other)?;

            cols_with_alias.push(ColumnStmt::new(col_name).alias(alias.map(|s| s.to_string())));
        }

        Ok(("", cols_with_alias))
    }

    fn parse_table_name(input: &str) -> IResult<&str, TableStmt> {
        // 解析 FROM table [AS] alias
        let (input, table_name) = delimited(
            (tag_no_case("FROM"), multispace0),
            alphanumeric1,
            multispace0,
        ).parse(input)?;

        // 可选别名：支持 "AS alias" 或 直接以空格跟别名
        let (input, alias_opt) = opt(
            preceded(
                alt((
                    delimited(multispace1, tag_no_case("AS"), multispace1), // AS alias
                    multispace1, // or just space then alias
                )),
                identifier,
            )
        ).parse(input)?;

        Ok((input, TableStmt {
            name: table_name.to_string(),
            alias: alias_opt.map(|s| s.to_string()),
        }))
    }

    fn parse_where_clause(input: &str) -> IResult<&str, Expr> {
        let (input, _) = (tag_no_case("WHERE"), multispace1).parse(input)?;
        let (input, where_expr) = parse_expr::parse(input)?;
        Ok((input, where_expr))
    }
}

mod parse_drop {
    use super::*;

    pub(super) fn parse(input: &str) -> IResult<&str, DropStmt> {
        let (input, _) = multispace0(input)?;
        trace!("开始解析DROP TABLE语句：{:?}", input);

        // 解析表名
        let (input, table_name) = identifier(input)?;
        let (input, _) = multispace0(input)?;
        trace!("解析到表名：{:?}", table_name);

        Ok((input, DropStmt {
            table_name: table_name.to_string(),
        }))
    }
}

mod parse_show {
    use super::*;

    pub(super) fn parse(input: &str) -> IResult<&str, ShowTablesStmt> {
        let (input, _) = multispace0(input)?;
        trace!("开始解析SHOW TABLES语句：{:?}", input);

        // SHOW TABLES语句不需要额外参数
        Ok((input, ShowTablesStmt {}))
    }
}

mod parse_insert {
    use super::*;
    use crate::stmt::InsertStmt;

    pub(super) fn parse(input: &str) -> IResult<&str, InsertStmt> {
        let (input, _) = multispace0(input)?;
        trace!("开始解析INSERT语句：{:?}", input);

        // 解析表名
        let (input, table_name) = identifier(input)?;
        let (input, _) = multispace0(input)?;
        trace!("解析到表名：{:?}", table_name);

        // 解析可选的列名列表
        let (input, columns) = opt(parse_column_list).parse(input)?;
        let (input, _) = multispace0(input)?;
        trace!("解析到列名列表：{:?}", columns);

        // 解析VALUES关键字
        let (input, _) = tag_no_case("VALUES")(input)?;
        let (input, _) = multispace0(input)?;

        // 解析值列表（支持多行插入）
        let (input, values) = separated_list1(
            delimited(multispace0, tag(","), multispace0),
            parse_value_row
        ).parse(input)?;
        trace!("解析到值列表：{:?}", values);

        Ok((input, InsertStmt {
            table_name: table_name.to_string(),
            columns: columns.map(|cols| cols.iter().map(|c| c.to_string()).collect()),
            values,
        }))
    }

    // 解析列名列表 (column1, column2, ...)
    fn parse_column_list(input: &str) -> IResult<&str, Vec<&str>> {
        delimited(
            tag("("),
            separated_list1(
                delimited(multispace0, tag(","), multispace0),
                preceded(multispace0, identifier)
            ),
            preceded(multispace0, tag(")"))
        ).parse(input)
    }

    // 解析单行值 ('value1', 'value2', ...)
    fn parse_value_row(input: &str) -> IResult<&str, Vec<String>> {
        delimited(
            tag("("),
            separated_list1(
                delimited(multispace0, tag(","), multispace0),
                parse_value
            ),
            preceded(multispace0, tag(")"))
        ).parse(input)
    }

    // 解析单个值（支持带引号的字符串、数字等）
    fn parse_value(input: &str) -> IResult<&str, String> {
        let (input, _) = multispace0(input)?;
        
        // 带引号的字符串
        let quoted_value = map(
            delimited(tag("'"), take_till1(|c| c == '\''), tag("'"))
                .or(delimited(tag("\""), take_till1(|c| c == '"'), tag("\""))),
            |s: &str| s.to_string()
        );
        
        // 不带引号的值（数字、NULL等）
        let unquoted_value = map(
            take_till1::<_, _, nom::error::Error<_>>(|c: char| c == ',' || c == ')' || c.is_whitespace()),
            |s: &str| s.to_string()
        );
        
        alt((quoted_value, unquoted_value)).parse(input)
    }
}

pub(crate) fn parse_sql_stmt(input: &str) -> IResult<&str, Stmt> {
    use StmtType::*;

    trace!("开始解析SQL语句：{:?}", input);

    // 解析不同类型的SQL语句
    let (input, _) = multispace0(input)?;
    let (input, stmt_type) = alt((
        value(Select, tag_no_case("SELECT")),
        value(Create, (tag_no_case("CREATE"), multispace1, tag_no_case("TABLE"))),
        value(Insert, (tag_no_case("INSERT"), multispace1, tag_no_case("INTO"))),
        value(Update, tag_no_case("UPDATE")),
        value(Delete, tag_no_case("DELETE")),
        value(Drop, (tag_no_case("DROP"), multispace1, tag_no_case("TABLE"))),
        value(ShowTables, (tag_no_case("SHOW"), multispace1, tag_no_case("TABLES"))),
    )).parse(input)?;

    trace!("解析到语句类型：{:?}", stmt_type);

    // 根据不同的语句类型调用相应的解析函数
    let (input, stmt_result) = match stmt_type {
        Select => map(parse_select::parse, |stmt| Stmt::Select(stmt)).parse(input),
        Create => map(parse_create::parse, |stmt| Stmt::Create(stmt)).parse(input),
        Drop => map(parse_drop::parse, |stmt| Stmt::Drop(stmt)).parse(input),
        ShowTables => map(parse_show::parse, |stmt| Stmt::ShowTables(stmt)).parse(input),
        Insert => map(parse_insert::parse, |stmt| Stmt::Insert(stmt)).parse(input),
        Update => unimplemented!("UPDATE parsing not implemented yet"),
        Delete => unimplemented!("DELETE parsing not implemented yet"),
    }?;

    Ok((input, stmt_result))
}

pub fn parse_sql(input: &str) -> Result<Stmt> {
    use crate::error::*;

    match parse_sql_stmt(input) {
        Ok((_, stmt)) => Ok(stmt),
        Err(err) => {
            error!("SQL解析错误：{:?}", err);
            Err(Error::Parser(ParserError::InvalidSyntax))
        }
    }
}