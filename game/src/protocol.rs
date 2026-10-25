use argon_dev::world::Player;
use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const PROTOCOL_VERSION: u32 = 1;
pub const PROTOCOL_ID: u64 = 0x4c4956494e470001;
// Shared development-only transport key. Session capabilities still authorize moves.
// Public Internet admission requires a trusted token issuer before production use.
pub const DEV_KEY: [u8; 32] = [42; 32];
pub const TICK: Duration = Duration::from_millis(16);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientCommand {
    Hello {
        protocol: u32,
        token: Option<String>,
    },
    Move {
        sequence: u64,
        dx: f64,
        dy: f64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerUpdate {
    Welcome {
        token: String,
        player: Player,
        next_sequence: u64,
    },
    Moved {
        player: Player,
        next_sequence: u64,
    },
    MoveRejected {
        reason: String,
        next_sequence: u64,
    },
    Snapshot {
        players: Vec<Player>,
        content_version: String,
        world_name: String,
        release: String,
        content_error: Option<String>,
    },
    Rejected {
        reason: String,
        reconnect: bool,
    },
}

pub struct CommandChannel;
pub struct UpdateChannel;
pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.register_message::<ClientCommand>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<ServerUpdate>()
            .add_direction(NetworkDirection::ServerToClient);
        app.add_channel::<CommandChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::ClientToServer);
        app.add_channel::<UpdateChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::ServerToClient);
    }
}
