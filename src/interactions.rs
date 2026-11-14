//! Message + button handling inside the dedicated player channel.

use std::sync::Arc;

use lavalink_rs::model::GuildId as LGuildId;
use serenity::all::{
    ComponentInteraction, CreateInteractionResponse, CreateInteractionResponseMessage, GuildId,
    Message,
};
use serenity::prelude::Context;

use crate::panel;
use crate::player::{ensure_player, resolve, tag_requester};
use crate::state::BotData;

/// Text typed in the player channel = a play request.
pub async fn on_message(ctx: &Context, msg: &Message, bot: &Arc<BotData>) {
    if msg.author.bot || msg.content.trim().is_empty() {
        return;
    }
    let Some(guild_id) = msg.guild_id else {
        return;
    };
    let is_player_channel = {
        let state = bot.shared.lock().await;
        state.is_player_channel(guild_id.get(), msg.channel_id.get())
    };
    if !is_player_channel {
        return;
    }

    let player = match ensure_player(ctx, bot, guild_id, msg.author.id, msg.channel_id).await {
        Ok(p) => p,
        Err(_) => {
            let _ = msg
                .channel_id
                .say(&ctx.http, "❌ Masuk ke voice channel dulu ya")
                .await;
            return;
        }
    };

    let (mut tracks, _playlist) = match resolve(bot, guild_id, msg.content.trim()).await {
        Ok(t) => t,
        Err(_) => {
            let _ = msg
                .channel_id
                .say(&ctx.http, "❌ Lagu tidak ditemukan")
                .await;
            return;
        }
    };

    tag_requester(&mut tracks, msg.author.id.get());

    let queue = player.get_queue();
    let was_empty = queue.get_count().await.unwrap_or(0) == 0;
    let playing = player
        .get_player()
        .await
        .map(|p| p.track.is_some())
        .unwrap_or(false);

    let _ = queue.append(tracks.into_iter().map(Into::into).collect());

    // Keep the channel tidy: the request text itself is not needed once queued.
    let _ = msg.delete(&ctx.http).await;

    if !playing && was_empty {
        let _ = player.skip();
    } else {
        panel::update(&bot.lavalink, &bot.shared, &ctx.http, guild_id).await;
    }
}

/// Panel buttons.
pub async fn on_component(ctx: &Context, cmp: &ComponentInteraction, bot: &Arc<BotData>) {
    let Some(guild_id) = cmp.guild_id else {
        return;
    };
    let Some(action) = cmp.data.custom_id.strip_prefix(panel::PREFIX) else {
        return;
    };

    let player = bot
        .lavalink
        .get_player_context(LGuildId(guild_id.get()));

    let mut toast = None::<String>;

    match (action, &player) {
        ("toggle", Some(p)) => {
            let paused = p.get_player().await.map(|d| d.paused).unwrap_or(false);
            let _ = p.set_pause(!paused).await;
        }
        ("next", Some(p)) => {
            let _ = p.skip();
        }
        ("prev", Some(p)) => {
            let last = bot.shared.lock().await.history.remove(&guild_id.get()).flatten();
            match last {
                Some(track) => {
                    let _ = p.get_queue().push_to_front(track);
                    let _ = p.skip();
                }
                None => toast = Some("Belum ada lagu sebelumnya".into()),
            }
        }
        ("stop", Some(p)) => {
            let _ = p.get_queue().clear();
            let _ = p.stop_now().await;
        }
        ("shuffle", Some(p)) => {
            if let Ok(mut items) = p.get_queue().get_queue().await {
                crate::commands::music::fastrand_shuffle(&mut items);
                let _ = p.get_queue().replace(items);
            }
        }
        ("voldown" | "volup", Some(p)) => {
            if let Ok(d) = p.get_player().await {
                let vol = if action == "volup" {
                    (d.volume as i32 + 10).min(200)
                } else {
                    (d.volume as i32 - 10).max(0)
                } as u16;
                let _ = p.set_volume(vol).await;
            }
        }
        ("loop", _) => {
            let mut state = bot.shared.lock().await;
            let s = state.guilds.entry(guild_id.get()).or_default();
            s.loop_mode = s.loop_mode.next();
        }
        ("autoplay", _) => {
            let mut state = bot.shared.lock().await;
            let s = state.guilds.entry(guild_id.get()).or_default();
            s.autoplay = !s.autoplay;
        }
        _ => {}
    }

    // The panel itself is edited below; the interaction only needs an acknowledgement.
    let ack = CreateInteractionResponseMessage::new();
    let response = match toast {
        Some(t) => CreateInteractionResponse::Message(ack.content(t)),
        None => CreateInteractionResponse::Defer(ack),
    };
    let _ = cmp.create_response(&ctx.http, response).await;

    panel::update(&bot.lavalink, &bot.shared, &ctx.http, guild_id).await;
}

/// Remember the finished track so ⏮️ can replay it.
pub async fn remember(
    shared: &Arc<tokio::sync::Mutex<crate::state::SharedState>>,
    guild_id: GuildId,
    track: lavalink_rs::model::track::TrackData,
) {
    shared
        .lock()
        .await
        .history
        .insert(guild_id.get(), Some(track));
}
