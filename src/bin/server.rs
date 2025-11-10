use futures_util::{StreamExt, SinkExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use env_logger::Env;
use log::{info, warn};

use lishdb::{DbHandler, HandleResult};

async fn handle_client(stream: TcpStream) -> anyhow::Result<()> {
    let ws_stream = accept_async(stream).await?;
    info!("客户端已连接");

    // 初始化数据库处理程序
    let db_handler = DbHandler::default();

    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    while let Some(msg) = ws_receiver.next().await {
        let msg = msg?;
        if msg.is_text() || msg.is_binary() {
            info!("收到消息: {}", msg);

            // 先直接解析 SQL 语句
            match db_handler.handle(msg.to_text()?).await {
                Ok(result) => {
                    match result {
                        HandleResult::Table(table) => {
                            let data = serde_binary::to_vec(&table, serde_binary::binary_stream::Endian::Big)?;

                            // 回显查询结果给客户端
                            ws_sender.send(Message::Binary(data.into())).await?;
                        },
                        HandleResult::Message(msg) => {
                            // 成功执行，回显给客户端
                            ws_sender.send(Message::Text(msg.into())).await?;
                        },
                    }
                }
                Err(e) => {
                    // 根据错误类型提供更准确的错误消息
                    match e {
                        lishdb::error::Error::Table(_) => {
                            // 表相关错误，直接显示错误消息
                            warn!("{}", e);
                            ws_sender.send(Message::Text(e.to_string().into())).await?;
                        },
                        lishdb::error::Error::Parser(_) => {
                            // 解析错误
                            let error_msg = format!("无法解析的 SQL 语句: {}", e);
                            warn!("{}", error_msg);
                            ws_sender.send(Message::Text(error_msg.into())).await?;
                        },
                        _ => {
                            // 其他错误
                            let error_msg = format!("执行错误: {}", e);
                            warn!("{}", error_msg);
                            ws_sender.send(Message::Text(error_msg.into())).await?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();

    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(addr).await?;
    info!("WebSocket 服务端运行在: {}", addr);

    tokio::spawn(async {
        tokio::signal::ctrl_c().await.unwrap();
        println!("\n👋 Ctrl-C 收到，立即退出");
        std::process::exit(0);
    });
    
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(handle_client(stream));
    }

    Ok(())
}