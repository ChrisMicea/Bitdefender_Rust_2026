mod game_algorithm;
mod protocol;

use anyhow::Context;
use futures_util::{SinkExt, StreamExt}; // stream::SplitSink
use serde::{Deserialize, Serialize};
// use serde_json::from_value;
// use std::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message}; // MaybeTlsStream, WebSocketStream

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
    Move,
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

#[tokio::main]
async fn main() {
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
                println!("You are ready to play!");

                // send Practice or Challenge - for now, Practice
                if let Err(e) = send_command(
                    &mut write,
                    ClientMessage {
                        command: ClientCommand::Practice,
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
            ServerCommand::StartMatch => {
                // start_args = Some(serde_json::from_value::<StartMatchArgs>(received_message.args).unwrap());
                let start_args =
                    serde_json::from_value::<StartMatchArgs>(received_message.args).unwrap();
                game_data.initialize_game(
                    start_args.config,
                    start_args.state,
                    start_args.your_player_id,
                );
                println!("\n\n\ninitialized game map\n\n");
            }
            ServerCommand::StartTurn => {
                // update game_state field inside game_data struct according to turn_args
                let turn_args: StartTurnArgs =
                    serde_json::from_value(received_message.args).unwrap();
                game_data.update_game_state(turn_args.state);

                let mut orders: Vec<ClientMessage> = Vec::new();

                let move_commands: Vec<MoveArgs> = game_data.move_heroes();
                for mv_cmd in move_commands {
                    orders.push(ClientMessage {
                        command: ClientCommand::Move,
                        args: serde_json::to_value(mv_cmd).unwrap(),
                    });
                }

                // respond with 2 commands: move or shoot
                // let mut move_command : [MoveArgs; 2] = [
                //     MoveArgs {
                //         hero_id: player_id * 2,
                //         x: 0,
                //         y: 0,
                //     },
                //     MoveArgs {
                //         hero_id: player_id * 2 + 1,
                //         x: 0,
                //         y: 0
                //     }
                // ];
                // for mv_cmd in move_command {
                //     orders.push(ClientMessage {
                //         command: ClientCommand::Move,
                //         args: serde_json::to_value(mv_cmd).unwrap(),
                //     });
                // }

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
            }
        }
    }
}
