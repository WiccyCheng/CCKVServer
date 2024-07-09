use anyhow::Result;
use futures::StreamExt;
use kv::{
    start_quic_client_with_config, start_yamux_client_with_config, AppStream, ClientConfig,
    CommandRequest, NetworkType,
};
use rustyline::{error::ReadlineError, DefaultEditor};
use std::collections::HashMap;
use tokio::io::{AsyncRead, AsyncWrite};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let config: ClientConfig = toml::from_str(include_str!("../fixtures/quic_client.conf"))?;

    // 打开一个 multiplex conn
    match config.general.network {
        NetworkType::Tcp => {
            let conn = start_yamux_client_with_config(&config).await?;
            process(conn).await?;
        }
        NetworkType::Quic => {
            let conn = start_quic_client_with_config(&config).await?;
            process(conn).await?;
        }
    }

    println!("Done!");

    Ok(())
}

async fn process<S, T>(mut conn: S) -> Result<()>
where
    S: AppStream<InnerStream = T>,
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut client = conn.open_stream().await?;
    let mut editor = DefaultEditor::new()?;
    if editor.load_history("history.txt").is_ok() {
        println!("History is loaded.");
    }

    let mut current_table = "0".to_string();
    let mut topic_map = HashMap::new();
    loop {
        let table = current_table.clone();
        let prompt = format!("kvserver[{}]> ", table.clone());
        let readline = editor.readline(&prompt);
        match readline {
            Ok(line) => {
                editor.add_history_entry(line.as_str())?;
                let args: Vec<&str> = line.split_whitespace().collect();
                if args.is_empty() {
                    continue;
                }
                let command = args[0].to_lowercase();
                match command.as_str() {
                    // kv command
                    "get" => {
                        if args.len() < 2 {
                            println!("Usage: GET <key>");
                            continue;
                        }

                        let cmd = CommandRequest::new_hget(table, args[1]);
                        let data = client.execute_unary(&cmd).await?;
                        println!("{data}");
                    }
                    "set" => {
                        if args.len() < 3 {
                            println!("Usage: SET <key> <value>");
                            continue;
                        }

                        let cmd = CommandRequest::new_hset(table, args[1], args[2]);
                        let data = client.execute_unary(&cmd).await?;
                        println!("{data}");
                    }
                    "del" => {
                        if args.len() < 2 {
                            println!("Usage: DEL <key>");
                            continue;
                        }

                        let cmd = CommandRequest::new_hdel(table, args[1]);
                        let data = client.execute_unary(&cmd).await?;
                        println!("{data}");
                    }
                    "exist" => {
                        if args.len() < 2 {
                            println!("Usage: EXIST <key>");
                            continue;
                        }

                        let cmd = CommandRequest::new_hexist(table, args[1]);
                        let data = client.execute_unary(&cmd).await?;
                        println!("{data}");
                    }
                    "select" => {
                        if args.len() < 2 {
                            println!("Usage: SELECT <table>");
                            continue;
                        }

                        current_table = args[1].to_string();
                    }

                    // chat command
                    "subscribe" => {
                        if args.len() < 2 {
                            println!("Usage: SUBSCRIBE <topic>");
                            continue;
                        }

                        let cmd = CommandRequest::new_subscribe(args[1]);
                        let client = conn.open_stream().await?;
                        let mut stream = client.execute_streaming(&cmd).await.unwrap();
                        topic_map.insert(args[1].to_owned(), stream.id);
                        tokio::spawn(async move {
                            while let Some(Ok(data)) = stream.next().await {
                                println!("Got published {data:?}",);
                            }
                        });
                    }
                    "unsubscribe" => {
                        if args.len() < 2 {
                            println!("Usage: UNSUBSCRIBE <topic>");
                            continue;
                        }

                        if let Some(id) = topic_map.remove(args[1]) {
                            let cmd = CommandRequest::new_unsubscribe(args[1], id);
                            let data = client.execute_unary(&cmd).await?;
                            println!("{data}");
                        } else {
                            println!("topic not exist");
                            continue;
                        }
                    }
                    "publish" => {
                        if args.len() < 3 {
                            println!("Usage: PUBLISH <topic> <value>");
                            continue;
                        }

                        let cmd = CommandRequest::new_publish(args[1], vec![args[2].into()]);
                        let data = client.execute_unary(&cmd).await?;
                        println!("{data}");
                    }

                    "quit" | "exit" => {
                        println!("Exiting...");
                        break;
                    }
                    _ => println!("Unknown command: {command}"),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}
