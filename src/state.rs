use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use lavalink_rs::prelude::LavalinkClient;
use serenity::gateway::ShardManager;
use serenity::prelude::*;
use tokio::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LoopMode {
    #[default]
    Off,
    Track,
    Queue,
}

impl LoopMode {
    pub fn label(self) -> &'static str {
        match self {
            LoopMode::Off => "off",
            LoopMode::Track => "track",
            LoopMode::Queue => "queue",
        }
    }

    /// Cycle off -> track -> queue -> off.
    pub fn next(self) -> Self {
        match self {
            LoopMode::Off => LoopMode::Track,
            LoopMode::Track => LoopMode::Queue,
            LoopMode::Queue => LoopMode::Off,
        }
    }
}

#[derive(Clone, Default)]
pub struct GuildSettings {
    pub loop_mode: LoopMode,
    pub autoplay: bool,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaylistTrack {
    pub title: String,
    pub author: String,
    pub uri: Option<String>,
    pub encoded: String,
}

/// The guild's dedicated player channel + the single message edited on every change.
#[derive(Clone, Copy)]
pub struct Panel {
    pub channel_id: u64,
    pub message_id: u64,
}

#[derive(Default)]
pub struct SharedState {
    pub guilds: HashMap<u64, GuildSettings>,
    /// guild -> playlist name -> tracks
    pub playlists: HashMap<u64, HashMap<String, Vec<PlaylistTrack>>>,
    /// guild -> dedicated player channel & panel message
    pub panels: HashMap<u64, Panel>,
    /// guild -> last finished track (for the ⏮️ button)
    pub history: HashMap<u64, Option<lavalink_rs::model::track::TrackData>>,
}

impl SharedState {
    pub fn settings(&self, guild_id: u64) -> GuildSettings {
        self.guilds.get(&guild_id).cloned().unwrap_or_default()
    }

    /// True when `channel_id` is the guild's dedicated player channel.
    pub fn is_player_channel(&self, guild_id: u64, channel_id: u64) -> bool {
        self.panels
            .get(&guild_id)
            .is_some_and(|p| p.channel_id == channel_id)
    }
}

/// Handed to `LavalinkClient::new_with_data`, readable from hooks via `client.data()`.
pub struct LavaState {
    pub shared: Arc<Mutex<SharedState>>,
    pub http: Arc<serenity::http::Http>,
}

/// Serenity TypeMap entry used by slash commands.
pub struct BotData {
    pub start_time: Instant,
    pub shard_manager: Mutex<Option<Arc<ShardManager>>>,
    pub lavalink: LavalinkClient,
    pub shared: Arc<Mutex<SharedState>>,
}

impl TypeMapKey for BotData {
    type Value = Arc<BotData>;
}
