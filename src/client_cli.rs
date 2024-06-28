use anyhow::Result;
use kv::{CommandRequest, ProstClientStream, TlsClientConnector};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let addr = "127.0.0.1:9527";

    let ca_cert = Some(include_str!("../fixtures/ca.cert"));
    let client_identity = Some((
        include_str!("../fixtures/client.cert"),
        include_str!("../fixtures/client.key"),
    ));

    let connector = TlsClientConnector::new("kvserver.acme.inc", client_identity, ca_cert)?;
    let stream = TcpStream::connect(addr).await?;
    let stream = connector.connect(stream).await?;
    let mut client = ProstClientStream::new(stream);

    let mut editor = DefaultEditor::new()?;
    if editor.load_history("history.txt").is_ok() {
        println!("History is loaded.");
    }

    let mut current_table = "default".to_string();
    loop {
        let table = current_table.clone();
        let prompt = format!("kvserver[table:{}]> ", table.clone());
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
                    "get" => {
                        if args.len() < 2 {
                            println!("Usage: GET <key>");
                            continue;
                        }

                        let cmd = CommandRequest::new_hget(table, args[1]);
                        let data = client.execute(cmd).await?;
                        println!("{data}");
                    }
                    "set" => {
                        if args.len() < 3 {
                            println!("Usage: SET <key> <value>");
                            continue;
                        }

                        let cmd = CommandRequest::new_hset(table, args[1], args[2]);
                        let data = client.execute(cmd).await?;
                        println!("{data}");
                    }
                    "del" => {
                        if args.len() < 2 {
                            println!("Usage: DEL <key>");
                            continue;
                        }

                        let cmd = CommandRequest::new_hdel(table, args[1]);
                        let data = client.execute(cmd).await?;
                        println!("{data}");
                    }
                    "exist" => {
                        if args.len() < 2 {
                            println!("Usage: EXIST <key>");
                            continue;
                        }

                        let cmd = CommandRequest::new_hexist(table, args[1]);
                        let data = client.execute(cmd).await?;
                        println!("{data}");
                    }
                    "select" => {
                        if args.len() < 2 {
                            println!("Usage: SELECT <table>");
                            continue;
                        }

                        current_table = args[1].to_string();
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
