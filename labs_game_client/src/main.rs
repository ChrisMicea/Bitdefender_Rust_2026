mod game_algorithm;
mod protocol;

use anyhow::Context;
use futures_util::{SinkExt, StreamExt}; // stream::SplitSink
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
// use serde_json::from_value;
// use std::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_tungstenite::tungstenite::stream::Mode;
// MaybeTlsStream, WebSocketStream

// use crate::game_algorithm::GameData;
use crate::protocol::{EndMatchArgs, MoveArgs, ShootArgs, StartMatchArgs, StartTurnArgs}; // Player

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerMessage {
    command: ServerCommand,
    args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientMessage {
    command: ClientCommand,
    args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServerCommand {
    Hello,
    Error,
    Ready,
    StartMatch,
    StartTurn,
    EndMatch,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClientCommand {
    Login,
    Practice,
    Challenge,
    Move,
    Shoot,
}

// CLI helpers VVVVVVVVVVVVVVVV

// What the user chose before the match starts.
#[derive(Debug, Clone)]
enum GameMode {
    Practice { my_id: i32 },
    Challenge { opponent: Option<String>, ranked: bool },
}

fn prompt(msg: &str) -> String {
    print!("{}", msg);
    io::stdout().flush().unwrap();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).unwrap();
    buf.trim().to_owned()
}

fn prompt_u32(msg: &str) -> Option<u32> {
    let s = prompt(msg);
    if s.is_empty() { None } else { s.parse().ok() }
}

fn ask_game_mode() -> GameMode {
    loop {
        println!("Select mode:");
        println!("1) Practice");
        println!("2) Challenge");
        let choice = prompt("Choice [1/2]: ");
        match choice.as_str() {
            "1" => {
                println!();
                println!("Starting position:");
                println!("0) Top of the map");
                println!("1) Bottom of the map");
                let pos_str = prompt("Position [0/1, default 0]: ");
                let my_id: i32 = match pos_str.as_str() {
                    "1" => 1,
                    _ => 0,
                };
                println!("Practice mode, position {}", if my_id == 0 { "top (0)" } else { "bottom (1)" });
                return GameMode::Practice { my_id };
            }
            "2" => {
                println!();
                let opp = prompt("Opponent name (leave blank for open matchmaking): ");
                let opponent = if opp.is_empty() { None } else { Some(opp) };

                let ranked_str = prompt("Ranked? [y/N]: ");
                let ranked = matches!(ranked_str.to_lowercase().as_str(), "y" | "yes");

                println!(
                    "Challenge mode — {} — {}",
                    opponent.as_deref().unwrap_or("open"),
                    if ranked { "ranked" } else { "unranked" }
                );
                return GameMode::Challenge { opponent, ranked };
            }
            _ => println!("Please enter 1 or 2.\n"),
        }
    }
}


async fn send_command<
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
>(
    write: &mut S,
    msg: ClientMessage,
) -> anyhow::Result<()> {
    let msg_deserialized = serde_json::to_string(&msg).context("serialize message")?;
    write
        .send(Message::Text(msg_deserialized.into()))
        .await
        .context("send message")?;
    Ok(())
}

async fn get_hero_ids() {}

async fn run_match(mode: GameMode) -> bool {
    let url = "wss://bitdefenders.cvjd.me/ws";
    let (ws, _) = connect_async(url).await.unwrap();
    let (mut write, mut read) = ws.split();

    println!("connected");

    let mut game_data = game_algorithm::GameData::default();

    while let Some(msg) = read.next().await {
        let msg = msg.unwrap();

        let text = match msg {
            Message::Text(text) => text,
            Message::Ping(payload) => {
                write.send(Message::Pong(payload)).await.unwrap();
                continue;
            }
            Message::Pong(_) => {
                println!("pong");
                continue;
            }
            Message::Binary(_) => {
                println!("binary message ignored");
                continue;
            }
            Message::Close(frame) => {
                println!("closed: {frame:?}");
                break;
            }
            Message::Frame(_) => continue,
        };

        let received_message: ServerMessage = serde_json::from_str(&text).unwrap();
        println!("{received_message:?}");
        match received_message.command {
            ServerCommand::Hello => {
                // Send login
                if let Err(e) = send_command(
                    &mut write,
                    ClientMessage {
                        command: ClientCommand::Login,
                        args: serde_json::json!({"version": 1, "name": "christian-micea-bot"}),
                    },
                )
                    .await
                {
                    println!("Failed to send login command: {e}");
                    break;
                }
            }
            ServerCommand::Error => {
                println!("Error: {received_message:?}");
                break;
            }

            ServerCommand::Ready => {
                println!("[ready — sending mode command]");
                let result = match &mode {
                    GameMode::Practice { my_id } => {
                        send_command(
                            &mut write,
                            ClientMessage {
                                command: ClientCommand::Practice,
                                args: serde_json::json!({ "my_id": my_id }),
                            },
                        )
                            .await
                    }
                    GameMode::Challenge { opponent, ranked } => {
                        let mut args = serde_json::json!({ "ranked": ranked });
                        if let Some(name) = opponent {
                            args["name"] = serde_json::Value::String(name.clone());
                        }
                        send_command(
                            &mut write,
                            ClientMessage {
                                command: ClientCommand::Challenge,
                                args,
                            },
                        )
                            .await
                    }
                };

                if let Err(e) = result {
                    eprintln!("Failed to send mode command: {e}");
                    break;
                }
            }

            ServerCommand::StartMatch => {
                // start_args = Some(serde_json::from_value::<StartMatchArgs>(received_message.args).unwrap());
                let start_args =
                    serde_json::from_value::<StartMatchArgs>(received_message.args).unwrap();
                game_data.initialize_game(
                    start_args.config,
                    start_args.state,
                    start_args.your_player_id,
                );
                println!("\n\ninitialized game map\n\n");
            }
            ServerCommand::StartTurn => {
                // update game_state field inside game_data struct according to turn_args
                let turn_args: StartTurnArgs =
                    serde_json::from_value(received_message.args).unwrap();
                game_data.update_game_state(turn_args.state);

                let mut orders: Vec<ClientMessage> = Vec::new();

                let (move_commands, shoot_commands) = game_data.decide_actions();
                for mv_cmd in move_commands {
                    orders.push(ClientMessage {
                        command: ClientCommand::Move,
                        args: serde_json::to_value(mv_cmd).unwrap(),
                    });
                }
                for shoot_cmd in shoot_commands {
                    orders.push(ClientMessage {
                        command: ClientCommand::Shoot,
                        args: serde_json::to_value(shoot_cmd).unwrap(),
                    });
                }

                let ws_messages = orders
                    .into_iter()
                    .map(|o| Message::Text(serde_json::to_string(&o).unwrap().into()))
                    .collect::<Vec<_>>();
                if let Err(e) = write
                    .send_all(&mut futures::stream::iter(ws_messages).map(Ok))
                    .await
                {
                    println!("Error sending messages: {}", e);
                }
            }
            ServerCommand::EndMatch => {
                // println!("Match has ended!! YIPEEEE!!!!");
                let end_args: EndMatchArgs = serde_json::from_value(received_message.args).unwrap();

                println!("The reason for ending the match: {}", end_args.reason);

                if let Some(winner) = &end_args.winner {
                    println!("The winner is: {}", winner)
                } else {
                    println!("There is no winner.")
                }

                break; // avoid infinite loop / hangup after a match ends so you can replay
            }
        }
    }

    return true;
}

#[tokio::main]
async fn main() {
    loop {
        let mode = ask_game_mode();
        run_match(mode).await;

        // Post-match prompt
        println!();
        loop {
            let choice = prompt("Play again? [y/N]: ");
            match choice.to_lowercase().as_str() {
                "y" | "yes" => break, // go back to outer loop -> new mode selection
                "n" | "no" | "" => {
                    println!("Goodbye!");
                    return;
                }
                _ => println!("Please enter y or n."),
            }
        }
    }
}
