//! `/play` autocomplete: live search suggestions while the user types.
//!
//! Discord asks us on every keystroke and only allows 3 seconds, so results are
//! cached per query string. Identical prefixes (users type letter by letter) are
//! then served instantly from memory instead of hitting the Lavalink node again.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use lavalink_rs::model::GuildId as LGuildId;
use serenity::builder::{CreateAutocompleteResponse, CreateInteractionResponse};
use serenity::model::application::CommandInteraction;
use serenity::prelude::Context;
use tokio::sync::Mutex;

use crate::format::fmt_ms;
use crate::state::BotData;

/// How long a query's suggestions stay cached.
const TTL: Duration = Duration::from_secs(300);
/// Don't search for one or two characters; too noisy and slow.
const MIN_CHARS: usize = 2;
/// Discord's hard limit.
const MAX_CHOICES: usize = 25;
/// A little under Discord's 3s budget, so a slow node never looks like a crash.
const SEARCH_TIMEOUT: Duration = Duration::from_millis(2200);

struct Entry {
    at: Instant,
    choices: Vec<(String, String)>,
}

#[derive(Default)]
pub struct SuggestionCache {
    map: Mutex<HashMap<String, Entry>>,
}

impl SuggestionCache {
    async fn get(&self, query: &str) -> Option<Vec<(String, String)>> {
        let map = self.map.lock().await;
        let entry = map.get(query)?;
        (entry.at.elapsed() < TTL).then(|| entry.choices.clone())
    }

    async fn put(&self, query: String, choices: Vec<(String, String)>) {
        let mut map = self.map.lock().await;
        map.retain(|_, e| e.at.elapsed() < TTL);
        map.insert(
            query,
            Entry {
                at: Instant::now(),
                choices,
            },
        );
    }
}

/// Handle an autocomplete interaction for `/play query`.
pub async fn on_autocomplete(ctx: &Context, cmd: &CommandInteraction, bot: &Arc<BotData>) {
    if cmd.data.name != "play" {
        return;
    }

    let Some(option) = cmd.data.autocomplete() else {
        return;
    };
    let query = option.value.trim().to_string();
    // Lavalink assigns nodes per guild, so search on behalf of the guild we are in.
    let guild_id = cmd.guild_id.unwrap_or(serenity::all::GuildId::new(0));

    let choices = suggestions(bot, guild_id, &query).await;
    let response = CreateInteractionResponse::Autocomplete(
        choices
            .into_iter()
            .fold(CreateAutocompleteResponse::new(), |b, (name, value)| {
                b.add_string_choice(name, value)
            }),
    );

    // Errors here are harmless: Discord drops the suggestions after 3 seconds.
    let _ = cmd.create_response(&ctx.http, response).await;
}

async fn suggestions(
    bot: &Arc<BotData>,
    guild_id: serenity::all::GuildId,
    query: &str,
) -> Vec<(String, String)> {
    if query.chars().count() < MIN_CHARS {
        return Vec::new();
    }
    // Links are unambiguous; no need to suggest anything for them.
    if query.starts_with("http://") || query.starts_with("https://") {
        return Vec::new();
    }

    if let Some(hit) = bot.suggestions.get(query).await {
        return hit;
    }

    let identifier = format!("ytsearch:{query}");
    let found = tokio::time::timeout(
        SEARCH_TIMEOUT,
        bot.lavalink.load_tracks(LGuildId(guild_id.get()), &identifier),
    )
    .await;

    let Ok(Ok(loaded)) = found else {
        return Vec::new();
    };

    let tracks = match loaded.data {
        Some(lavalink_rs::model::track::TrackLoadData::Search(v)) => v,
        Some(lavalink_rs::model::track::TrackLoadData::Track(t)) => vec![t],
        _ => Vec::new(),
    };

    let choices: Vec<(String, String)> = tracks
        .into_iter()
        .take(MAX_CHOICES)
        .map(|t| {
            // Discord labels are capped at 100 characters.
            let label = format!(
                "{} — {} [{}]",
                truncate(&t.info.title, 60),
                truncate(&t.info.author, 25),
                fmt_ms(t.info.length)
            );
            // The value is what `/play` receives: a stable URL beats a raw title.
            let value = t.info.uri.unwrap_or_else(|| t.info.title.clone());
            (truncate(&label, 100), value)
        })
        .collect();

    bot.suggestions.put(query.to_string(), choices.clone()).await;
    choices
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}
