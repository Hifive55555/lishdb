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
                    println!("{} {}", "<<<".bright_blue().bold(), back);
                    got_reply = true;
                }
                Some(Ok(Message::Binary(data))) => {
                    let table: TableActual = serde_binary::from_vec(data.to_vec(), serde_binary::binary_stream::Endian::Big)?;
                    println!("接收到表结果了！{:?}", table);
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    eprintln!("❌ 服务端错误: {}", e);
                    return Ok(());
                }
                None => {
                    println!("🏁 服务端关闭连接");
                    return Ok(());
                }
            }
        }
    }

    write.close().await?;
    Ok(())
}