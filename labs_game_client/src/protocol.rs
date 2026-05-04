// protocol/protocol.rs
//! Bit Defenders — WebSocket message envelope: `{ "command": string, "args": object }`.
//! All commands use JSON objects for `args` (may be empty `{}`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: i32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub command: String,
    #[serde(default)]
    pub args: serde_json::Value,
}

// --- Handshake & lobby ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloArgs {
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginArgs {
    pub name: String,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReadyArgs {}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChallengeArgs {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub seed: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PracticeArgs {
    #[serde(default)]
    pub seed: Option<u32>,
}

// --- Match ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: i32,
    pub name: String,
    pub heroes: Vec<PlayerHeroSpawn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerHeroSpawn {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    #[serde(rename = "type")]
    pub type_: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroTypeConfig {
    pub shoot_cooldown: i32,
    pub projectile_ttl: i32,
    pub projectile_speed: i32,
    pub max_hp: i32,
    pub projectile_damage: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub width: i32,
    pub height: i32,
    pub turns: i32,
    pub vision_range: i32,
    pub seed: u32,
    pub players: Vec<Player>,
    pub hero_types: HashMap<String, HeroTypeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hero {
    pub id: i32,
    pub owner_id: i32,
    #[serde(rename = "type")]
    pub type_: String,
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    pub cooldown: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Projectile {
    pub owner_id: i32,
    #[serde(rename = "type")]
    pub type_: String,
    pub origin_x: i32,
    pub origin_y: i32,
    pub x: i32,
    pub y: i32,
    pub ttl: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wall {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub heroes: Vec<Hero>,
    pub projectiles: Vec<Projectile>,
    pub walls: Vec<Wall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartMatchArgs {
    pub config: GameConfig,
    pub state: GameState,
    pub match_id: String,
    pub your_player_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartTurnArgs {
    pub turn: i32,
    pub state: GameState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveArgs {
    pub hero_id: i32,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShootArgs {
    pub hero_id: i32,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndMatchArgs {
    pub reason: String,
    #[serde(default)]
    pub winner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorArgs {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub fatal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PingArgs {}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PongArgs {}

/// Spectator / web UI: subscribe to match updates after optional HTTP discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchArgs {
    pub match_id: String,
}

