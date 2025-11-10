use anyhow::Context;
use colored::*;
use futures_util::{SinkExt, StreamExt};
use lishdb::TableActual;
use std::io::{self, Write};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 全局 Ctrl-C 钩子
    tokio::spawn(async {
        tokio::signal::ctrl_c().await.unwrap();
        println!("\n👋 Ctrl-C 收到，立即退出");
        std::process::exit(0);
    });

    let url = "ws://127.0.0.1:8080";
    let (ws_stream, _) = connect_async(url).await.context("连接失败")?;
    let (mut write, mut read) = ws_stream.split();

    // 2. 通道：stdin 任务 → 主循环
    let (stdin_tx, mut stdin_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    // 3. 异步读键盘（不带前缀）
    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let line = line.trim().to_string();
            if line == "quit" {
                break;
            }
            let _ = stdin_tx.send(line);
        }
    });

    // 4. 主循环：先发一条 → 等一条 → 再出现前缀
    loop {
        print!("{} ", "lishdb >".bright_green().bold());
        io::stdout().flush().unwrap();

        let text = match stdin_rx.recv().await {
            Some(t) => t,
            None => break,
        };

        write.send(Message::Text(text.into())).await?;

        // 等回包
        let mut got_reply = false;
        while !got_reply {
            match read.next().await {
                Some(Ok(Message::Text(back))) => {
                    println!("{}", back);
                    got_reply = true;
                }
                Some(Ok(Message::Binary(data))) => {
                    match bincode::deserialize::<TableActual>(&data) {
                        Ok(table) => {
                            println!("{}\n{}", "接收到表结果了！".bright_blue().bold(), table);
                        },
                        Err(e) => {
                            // 如果反序列化失败，尝试以文本形式打印数据
                            eprintln!("反序列化失败: {}", e);
                            println!("二进制数据长度: {} 字节", data.len());
                            // 打印部分数据用于调试
                            let display_data = &data[0..std::cmp::min(32, data.len())];
                            println!("数据前32字节: {:?}", display_data);
                        }
                    }
                    got_reply = true; // 设置为true，以便退出内部循环
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    eprintln!("{}\n{}", "❌ 服务端错误:".bright_red().bold(), e);
                    return Ok(());
                }
                None => {
                    println!("{}", "🏁 服务端关闭连接".yellow());
                    return Ok(());
                }
            }
        }
    }

    write.close().await?;
    Ok(())
}