//! The persistent "player" message: one embed + buttons per guild, edited in place.

use std::sync::Arc;

use lavalink_rs::model::GuildId as LGuildId;
use lavalink_rs::prelude::LavalinkClient;
use serenity::all::{
    ButtonStyle, ChannelId, CreateActionRow, CreateButton, CreateEmbed, CreateEmbedFooter,
    CreateMessage, EditMessage, GuildId, Http, MessageId, ReactionType,
};
use tokio::sync::Mutex;

use crate::format::{fmt_ms, progress_bar, truncate};
use crate::state::{LoopMode, SharedState};

/// `custom_id` prefix for every panel button.
pub const PREFIX: &str = "np:";

pub struct View {
    pub embed: CreateEmbed,
    pub rows: Vec<CreateActionRow>,
}

fn btn(id: &str, emoji: &str, style: ButtonStyle) -> CreateButton {
    CreateButton::new(format!("{PREFIX}{id}"))
        .emoji(ReactionType::Unicode(emoji.to_string()))
        .style(style)
}

/// Build the panel embed + buttons from live player state.
pub async fn view(
    lavalink: &LavalinkClient,
    shared: &Arc<Mutex<SharedState>>,
    guild_id: GuildId,
) -> View {
    let (loop_mode, autoplay) = {
        let state = shared.lock().await;
        let s = state.settings(guild_id.get());
        (s.loop_mode, s.autoplay)
    };

    let Some(player) = lavalink.get_player_context(LGuildId(guild_id.get())) else {
        return idle(loop_mode, autoplay);
    };

    let queue_len = player.get_queue().get_count().await.unwrap_or(0);
    let Ok(data) = player.get_player().await else {
        return idle(loop_mode, autoplay);
    };

    let Some(track) = data.track else {
        return idle(loop_mode, autoplay);
    };

    let info = &track.info;
    let (pos, total) = (data.state.position, info.length);
    let requester = track
        .user_data
        .as_ref()
        .and_then(|u| u.get("requester_id"))
        .and_then(|v| v.as_u64());

    let mut embed = CreateEmbed::new()
        .title(if data.paused {
            "⏸️ Paused"
        } else {
            "🎶 Now Playing"
        })
        .description(format!(
            "**[{}]({})**\n`{}` {} `{}`",
            truncate(&info.title, 80),
            info.uri.as_deref().unwrap_or(""),
            fmt_ms(pos),
            progress_bar(pos, total, 16),
            if info.is_stream {
                "LIVE".to_string()
            } else {
                fmt_ms(total)
            }
        ))
        .field("Artis", truncate(&info.author, 60), true)
        .field("Volume", format!("`{}%`", data.volume), true)
        .field("Antrean", format!("`{queue_len}` lagu"), true)
        .field("Loop", loop_mode.label(), true)
        .field("Autoplay", if autoplay { "aktif" } else { "off" }, true)
        .color(if data.paused { 0xe67e22 } else { 0x9b59b6 })
        .footer(CreateEmbedFooter::new(
            "Ketik judul lagu atau link di channel ini · Rust",
        ));

    if let Some(r) = requester {
        embed = embed.field("Diminta oleh", format!("<@{r}>"), true);
    }
    if let Some(art) = &info.artwork_url {
        embed = embed.thumbnail(art);
    }

    View {
        embed,
        rows: rows(data.paused, loop_mode, autoplay),
    }
}

fn idle(loop_mode: LoopMode, autoplay: bool) -> View {
    View {
        embed: CreateEmbed::new()
            .title("📭 Belum ada lagu")
            .description("Ketik judul lagu atau tempel link (YouTube, Spotify, SoundCloud) di channel ini.")
            .field("Loop", loop_mode.label(), true)
            .field("Autoplay", if autoplay { "aktif" } else { "off" }, true)
            .color(0x2c3e50)
            .footer(CreateEmbedFooter::new("Rust • Lavalink v4")),
        rows: rows(false, loop_mode, autoplay),
    }
}

fn rows(paused: bool, loop_mode: LoopMode, autoplay: bool) -> Vec<CreateActionRow> {
    let loop_style = if loop_mode == LoopMode::Off {
        ButtonStyle::Secondary
    } else {
        ButtonStyle::Success
    };

    vec![
        CreateActionRow::Buttons(vec![
            btn("prev", "⏮️", ButtonStyle::Secondary),
            btn("toggle", if paused { "▶️" } else { "⏸️" }, ButtonStyle::Primary),
            btn("next", "⏭️", ButtonStyle::Secondary),
            btn("stop", "⏹️", ButtonStyle::Danger),
            btn("shuffle", "🔀", ButtonStyle::Secondary),
        ]),
        CreateActionRow::Buttons(vec![
            btn("voldown", "🔉", ButtonStyle::Secondary),
            btn("volup", "🔊", ButtonStyle::Secondary),
            btn("loop", "🔁", loop_style),
            btn(
                "autoplay",
                "♾️",
                if autoplay {
                    ButtonStyle::Success
                } else {
                    ButtonStyle::Secondary
                },
            ),
        ]),
    ]
}

/// Edit the stored panel message in place. Returns false when no panel is configured.
pub async fn update(
    lavalink: &LavalinkClient,
    shared: &Arc<Mutex<SharedState>>,
    http: &Arc<Http>,
    guild_id: GuildId,
) -> bool {
    let panel = {
        let state = shared.lock().await;
        state.panels.get(&guild_id.get()).copied()
    };
    let Some(panel) = panel else {
        return false;
    };

    let view = view(lavalink, shared, guild_id).await;
    let _ = http
        .edit_message(
            ChannelId::new(panel.channel_id),
            MessageId::new(panel.message_id),
            &EditMessage::new()
                .embed(view.embed)
                .components(view.rows),
            Vec::new(),
        )
        .await;
    true
}

/// Post the first panel message in a channel and remember it.
pub async fn post(
    lavalink: &LavalinkClient,
    shared: &Arc<Mutex<SharedState>>,
    http: &Arc<Http>,
    guild_id: GuildId,
    channel_id: ChannelId,
) -> Result<MessageId, String> {
    let view = view(lavalink, shared, guild_id).await;
    let msg = channel_id
        .send_message(
            http,
            CreateMessage::new()
                .embed(view.embed)
                .components(view.rows),
        )
        .await
        .map_err(|e| format!("Gagal kirim panel: {e}"))?;

    shared.lock().await.panels.insert(
        guild_id.get(),
        crate::state::Panel {
            channel_id: channel_id.get(),
            message_id: msg.id.get(),
        },
    );

    Ok(msg.id)
}
