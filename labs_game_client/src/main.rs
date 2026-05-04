mod protocol;

use anyhow::Context;
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use serde::{Deserialize, Serialize};
use serde_json::from_value;
use std::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

use crate::protocol::{EndMatchArgs, MoveArgs, StartMatchArgs, StartTurnArgs};

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketMessage {
    command: Command,
    args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Command {
    Hello,
    Login,
    Error,
    Ready,
    Practice,
    StartMatch,
    StartTurn,
    EndMatch,
    Move,
}

async fn send_command<
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
>(
    write: &mut S,
    msg: WebSocketMessage,
) -> anyhow::Result<()> {
    let msg_deserialized = serde_json::to_string(&msg).context("serialize message")?;
    write
        .send(Message::Text(msg_deserialized.into()))
        .await
        .context("send message")?;
    Ok(())
}

async fn get_hero_ids() {

}

#[tokio::main]
async fn main() {
    let url = "wss://bitdefenders.cvjd.me/ws";
    let (ws, _) = connect_async(url).await.unwrap();
    let (mut write, mut read) = ws.split();

    println!("connected");

    // let mut start_args = None;
    let mut player_id = 0;

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

        let message: WebSocketMessage = serde_json::from_str(&text).unwrap();
        println!("{message:?}");
        match message.command {
            Command::Hello => {
                // Send login
                if let Err(e) = send_command(
                    &mut write,
                    WebSocketMessage {
                        command: Command::Login,
                        args: serde_json::json!({"version": 1, "name": "christian-micea-bot"}),
                    },
                )
                .await
                {
                    println!("Failed to send login command: {e}");
                    break;
                }
            }
            Command::Login => {
                // Login should be sent by the client to the server, not vice-versa
                panic!("What are you doing here?");
            }
            Command::Error => {
                println!("Error: {message:?}");
                break;
            }
            Command::Ready => {
                println!("You are ready to play!");

                // send Practice or Challenge - for now, Practice
                if let Err(e) = send_command(
                    &mut write,
                    WebSocketMessage {
                        command: Command::Practice,
                        args: serde_json::json!({}), // seed argument is optional
                                                     // args: serde_json::json!({"seed": 1})
                    },
                )
                .await
                {
                    println!("Failed to send Practice command: {e}");
                    break;
                }
            }
            Command::Practice => {
                // Practice should be sent by the client to the server, not vice-versa
                panic!("What are you doing here?");
            }
            Command::StartMatch => {
                // start_args = Some(serde_json::from_value::<StartMatchArgs>(message.args).unwrap());
                let start_args = serde_json::from_value::<StartMatchArgs>(message.args).unwrap();
                player_id = start_args.your_player_id;
            }
            Command::StartTurn => {
                // skip for now
                let turn_args: StartTurnArgs = serde_json::from_value(message.args).unwrap();
                
                // let start_args = start_args.as_ref().unwrap();
                // let Some(start_args) = &start_args else {
                //     panic!("am facut unwrapu de mana");
                // };
                
                // respond with 2 commands: move or shoot
                let mut move_command : [MoveArgs; 2] = [
                    MoveArgs {
                        hero_id: player_id * 2,
                        x: 0,
                        y: 0,
                    },
                    MoveArgs {
                        hero_id: player_id * 2 + 1,
                        x: 0,
                        y: 0
                    }
                ];

                for mv_cmd in move_command {
                    if let Err(e) = send_command(
                        &mut write,
                        WebSocketMessage {
                            command: Command::Move,
                            args: serde_json::to_value(mv_cmd).unwrap(),
                        },
                    )
                    .await
                    {
                        println!("Failed to send Practice command: {e}");
                        break;
                    }
                }
            }
            Command::Move => {
                // Move should be sent by the client to the server, not vice-versa
                panic!("What are you doing here?");
            }
            Command::EndMatch => {
                // println!("Match has ended!! YIPEEEE!!!!");
                let endArgs: EndMatchArgs = serde_json::from_value(message.args).unwrap();

                println!("The reason for ending the match: {}", endArgs.reason);

                if let Some(winner) = &endArgs.winner {
                    println!("The winner is: {}", winner)
                }
                else {
                    println!("There is no winner.")
                }
            }
        }
    }
}
