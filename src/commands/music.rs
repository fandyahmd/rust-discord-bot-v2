//! Music playback commands.

use std::collections::VecDeque;
use std::sync::Arc;

use lavalink_rs::model::track::TrackData;
use serenity::all::{CommandDataOptionValue, CreateEmbed, CreateEmbedFooter, EditInteractionResponse, GuildId};
use serenity::model::application::CommandInteraction;
use serenity::prelude::Context;

use crate::format::{fmt_ms, parse_time, progress_bar, truncate};
use crate::player::{ensure_player, resolve, tag_requester};
use crate::state::{BotData, LoopMode, PlaylistTrack};

/// Tuple passed to every handler.
pub type Args<'a> = (&'a Context, &'a CommandInteraction, &'a Arc<BotData>, GuildId);

pub async fn reply(ctx: &Context, cmd: &CommandInteraction, msg: impl Into<String>) {
    let _ = cmd
        .edit_response(&ctx.http, EditInteractionResponse::new().content(msg))
        .await;
}

async fn reply_embed(ctx: &Context, cmd: &CommandInteraction, embed: CreateEmbed) {
    let _ = cmd
        .edit_response(&ctx.http, EditInteractionResponse::new().embed(embed))
        .await;
}

fn opt_str<'a>(cmd: &'a CommandInteraction, name: &str) -> Option<&'a str> {
    cmd.data
        .options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| o.value.as_str())
}

fn opt_int(cmd: &CommandInteraction, name: &str) -> Option<i64> {
    cmd.data
        .options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| o.value.as_i64())
}

fn opt_bool(cmd: &CommandInteraction, name: &str) -> Option<bool> {
    cmd.data
        .options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| o.value.as_bool())
}

/// Convert owned tracks into queue entries.
fn to_queue(tracks: Vec<TrackData>) -> VecDeque<lavalink_rs::prelude::TrackInQueue> {
    tracks.into_iter().map(Into::into).collect()
}

pub async fn play((ctx, cmd, bot, guild_id): Args<'_>) {
    let Some(query) = opt_str(cmd, "query").map(str::to_string) else {
        return reply(ctx, cmd, "❌ Query kosong").await;
    };

    let player = match ensure_player(ctx, bot, guild_id, cmd.user.id, cmd.channel_id).await {
        Ok(p) => p,
        Err(e) => return reply(ctx, cmd, format!("❌ {e}")).await,
    };

    let (mut tracks, playlist_name) = match resolve(bot, guild_id, &query).await {
        Ok(t) => t,
        Err(e) => return reply(ctx, cmd, format!("❌ {e}")).await,
    };

    tag_requester(&mut tracks, cmd.user.id.get());
    let count = tracks.len();
    let first = tracks[0].info.title.clone();
    let first_author = tracks[0].info.author.clone();

    let queue = player.get_queue();
    let was_empty = queue.get_count().await.unwrap_or(0) == 0;
    let playing = player
        .get_player()
        .await
        .map(|p| p.track.is_some())
        .unwrap_or(false);

    let _ = queue.append(to_queue(tracks));

    let msg = match playlist_name {
        Some(n) => format!("📑 **{n}** — {count} lagu masuk antrean"),
        None => format!("🎵 **{}** · `{}` masuk antrean", truncate(&first, 60), first_author),
    };

    if !playing && was_empty {
        let _ = player.skip();
    }

    reply(ctx, cmd, msg).await
}

pub async fn pause_resume((ctx, cmd, bot, guild_id): Args<'_>, pause: bool) {
    let Some(player) = bot.lavalink.get_player_context(guild_id.get()) else {
        return reply(ctx, cmd, "❌ Tidak ada lagu yang diputar").await;
    };

    match player.set_pause(pause).await {
        Ok(_) if pause => reply(ctx, cmd, "⏸️ Dijeda").await,
        Ok(_) => reply(ctx, cmd, "▶️ Lanjut").await,
        Err(e) => reply(ctx, cmd, format!("❌ Gagal: {e}")).await,
    }
}

pub async fn skip((ctx, cmd, bot, guild_id): Args<'_>) {
    let Some(player) = bot.lavalink.get_player_context(guild_id.get()) else {
        return reply(ctx, cmd, "❌ Tidak ada lagu yang diputar").await;
    };

    let title = player
        .get_player()
        .await
        .ok()
        .and_then(|p| p.track)
        .map(|t| truncate(&t.info.title, 60))
        .unwrap_or_else(|| "lagu".into());

    let _ = player.skip();
    reply(ctx, cmd, format!("⏭️ Skip: **{title}**")).await
}

pub async fn stop((ctx, cmd, bot, guild_id): Args<'_>) {
    let Some(player) = bot.lavalink.get_player_context(guild_id.get()) else {
        return reply(ctx, cmd, "❌ Tidak ada lagu yang diputar").await;
    };

    let _ = player.get_queue().clear();
    let _ = player.stop_now().await;
    reply(ctx, cmd, "⏹️ Dihentikan, antrean dibersihkan").await
}

pub async fn queue((ctx, cmd, bot, guild_id): Args<'_>) {
    let Some(player) = bot.lavalink.get_player_context(guild_id.get()) else {
        return reply(ctx, cmd, "❌ Tidak ada lagu yang diputar").await;
    };

    let items = match player.get_queue().get_queue().await {
        Ok(q) => q,
        Err(e) => return reply(ctx, cmd, format!("❌ {e}")).await,
    };

    let total = items.len();
    if total == 0 {
        return reply(ctx, cmd, "📭 Antrean kosong").await;
    }

    const PER_PAGE: usize = 10;
    let pages = total.div_ceil(PER_PAGE);
    let page = (opt_int(cmd, "page").unwrap_or(1).max(1) as usize).min(pages);
    let start = (page - 1) * PER_PAGE;

    let list = items
        .iter()
        .enumerate()
        .skip(start)
        .take(PER_PAGE)
        .map(|(i, t)| {
            let info = &t.track.info;
            format!(
                "`{}.` [{}]({}) · `{}`",
                i + 1,
                truncate(&info.title, 45),
                info.uri.as_deref().unwrap_or(""),
                if info.is_stream {
                    "LIVE".to_string()
                } else {
                    fmt_ms(info.length)
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let embed = CreateEmbed::new()
        .title(format!("📋 Antrean — {total} lagu"))
        .description(list)
        .color(0xf1c40f)
        .footer(CreateEmbedFooter::new(format!("Halaman {page}/{pages}")));

    reply_embed(ctx, cmd, embed).await
}

pub async fn now_playing((ctx, cmd, bot, guild_id): Args<'_>) {
    let Some(player) = bot.lavalink.get_player_context(guild_id.get()) else {
        return reply(ctx, cmd, "❌ Tidak ada lagu yang diputar").await;
    };

    let data = match player.get_player().await {
        Ok(p) => p,
        Err(e) => return reply(ctx, cmd, format!("❌ {e}")).await,
    };

    let Some(track) = data.track else {
        return reply(ctx, cmd, "❌ Tidak ada lagu yang diputar").await;
    };

    let info = &track.info;
    let (pos, total) = (data.state.position, info.length);

    let mut embed = CreateEmbed::new()
        .title("🎧 Now Playing")
        .description(format!(
            "**[{}]({})**\n`{}` {} `{}`",
            info.title,
            info.uri.as_deref().unwrap_or(""),
            fmt_ms(pos),
            progress_bar(pos, total, 14),
            fmt_ms(total)
        ))
        .field("Artis", &info.author, true)
        .field("Volume", format!("`{}%`", data.volume), true)
        .field(
            "Status",
            if data.paused { "⏸️ Paused" } else { "▶️ Playing" },
            true,
        )
        .color(0x3498db)
        .footer(CreateEmbedFooter::new("Rust • Lavalink v4"));

    if let Some(art) = &info.artwork_url {
        embed = embed.thumbnail(art);
    }

    reply_embed(ctx, cmd, embed).await
}

pub async fn volume((ctx, cmd, bot, guild_id): Args<'_>) {
    let level = opt_int(cmd, "level").unwrap_or(100).clamp(0, 200) as u16;

    let Some(player) = bot.lavalink.get_player_context(guild_id.get()) else {
        return reply(ctx, cmd, "❌ Tidak ada lagu yang diputar").await;
    };

    match player.set_volume(level).await {
        Ok(_) => reply(ctx, cmd, format!("🔊 Volume: `{level}%`")).await,
        Err(e) => reply(ctx, cmd, format!("❌ Gagal: {e}")).await,
    }
}

pub async fn seek((ctx, cmd, bot, guild_id): Args<'_>) {
    let raw = opt_str(cmd, "position").unwrap_or("");

    let Some(dur) = parse_time(raw) else {
        return reply(ctx, cmd, format!("❌ Format waktu salah: `{raw}` (contoh `1:30`)")).await;
    };

    let Some(player) = bot.lavalink.get_player_context(guild_id.get()) else {
        return reply(ctx, cmd, "❌ Tidak ada lagu yang diputar").await;
    };

    match player.set_position(dur).await {
        Ok(_) => reply(ctx, cmd, format!("⏩ Seek ke `{}`", fmt_ms(dur.as_millis() as u64))).await,
        Err(e) => reply(ctx, cmd, format!("❌ Gagal seek: {e}")).await,
    }
}

pub async fn loop_mode((ctx, cmd, bot, guild_id): Args<'_>) {
    let requested = opt_str(cmd, "mode").map(str::to_string);

    let mode = {
        let mut state = bot.shared.lock().await;
        let settings = state.guilds.entry(guild_id.get()).or_default();
        settings.loop_mode = match requested.as_deref() {
            Some("off") => LoopMode::Off,
            Some("track") => LoopMode::Track,
            Some("queue") => LoopMode::Queue,
            _ => settings.loop_mode.next(),
        };
        settings.loop_mode
    };

    let msg = match mode {
        LoopMode::Off => "➡️ Loop: **off**",
        LoopMode::Track => "🔂 Loop: **track**",
        LoopMode::Queue => "🔁 Loop: **queue**",
    };
    reply(ctx, cmd, msg).await
}

pub async fn shuffle((ctx, cmd, bot, guild_id): Args<'_>) {
    let Some(player) = bot.lavalink.get_player_context(guild_id.get()) else {
        return reply(ctx, cmd, "❌ Tidak ada lagu yang diputar").await;
    };

    let queue = player.get_queue();
    let mut items = match queue.get_queue().await {
        Ok(q) => q,
        Err(e) => return reply(ctx, cmd, format!("❌ {e}")).await,
    };

    if items.len() < 2 {
        return reply(ctx, cmd, "❌ Antrean terlalu pendek untuk diacak").await;
    }

    let n = items.len();
    fastrand_shuffle(&mut items);
    let _ = queue.replace(items);
    reply(ctx, cmd, format!("🔀 {n} lagu diacak")).await
}

/// Fisher-Yates with a time-seeded LCG (no extra dependency).
pub fn fastrand_shuffle<T>(items: &mut VecDeque<T>) {
    use std::time::{SystemTime, UNIX_EPOCH};
    let mut seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9e3779b97f4a7c15);

    for i in (1..items.len()).rev() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let j = (seed >> 33) as usize % (i + 1);
        items.swap(i, j);
    }
}

pub async fn remove((ctx, cmd, bot, guild_id): Args<'_>) {
    let index = opt_int(cmd, "index").unwrap_or(1).max(1) as usize - 1;

    let Some(player) = bot.lavalink.get_player_context(guild_id.get()) else {
        return reply(ctx, cmd, "❌ Tidak ada lagu yang diputar").await;
    };

    let queue = player.get_queue();
    let track = match queue.get_track(index).await {
        Ok(Some(t)) => t,
        Ok(None) => return reply(ctx, cmd, format!("❌ Tidak ada lagu nomor {}", index + 1)).await,
        Err(e) => return reply(ctx, cmd, format!("❌ {e}")).await,
    };

    let _ = queue.remove(index);
    let title = truncate(&track.track.info.title, 60);
    reply(ctx, cmd, format!("🗑️ Dihapus: **{title}**")).await
}

pub async fn move_track((ctx, cmd, bot, guild_id): Args<'_>) {
    let from = opt_int(cmd, "from").unwrap_or(1).max(1) as usize - 1;
    let to = opt_int(cmd, "to").unwrap_or(1).max(1) as usize - 1;

    let Some(player) = bot.lavalink.get_player_context(guild_id.get()) else {
        return reply(ctx, cmd, "❌ Tidak ada lagu yang diputar").await;
    };

    let queue = player.get_queue();
    let mut items = match queue.get_queue().await {
        Ok(q) => q,
        Err(e) => return reply(ctx, cmd, format!("❌ {e}")).await,
    };

    if from >= items.len() || to >= items.len() {
        return reply(ctx, cmd, "❌ Nomor antrean di luar jangkauan").await;
    }

    if let Some(t) = items.remove(from) {
        items.insert(to, t);
    }
    let _ = queue.replace(items);
    reply(ctx, cmd, format!("↔️ Dipindah dari `{}` ke `{}`", from + 1, to + 1)).await
}

pub async fn clear((ctx, cmd, bot, guild_id): Args<'_>) {
    let Some(player) = bot.lavalink.get_player_context(guild_id.get()) else {
        return reply(ctx, cmd, "❌ Tidak ada lagu yang diputar").await;
    };

    let q = player.get_queue();
    let n = q.get_count().await.unwrap_or(0);
    let _ = q.clear();
    reply(ctx, cmd, format!("🧹 {n} lagu dihapus dari antrean")).await
}

pub async fn autoplay((ctx, cmd, bot, guild_id): Args<'_>) {
    let enabled = opt_bool(cmd, "enabled").unwrap_or(false);

    bot.shared
        .lock()
        .await
        .guilds
        .entry(guild_id.get())
        .or_default()
        .autoplay = enabled;

    reply(
        ctx,
        cmd,
        if enabled {
            "🎲 Autoplay: **aktif**"
        } else {
            "🎲 Autoplay: **nonaktif**"
        },
    )
    .await
}

pub async fn filter((ctx, cmd, bot, guild_id): Args<'_>) {
    use lavalink_rs::model::player::{Equalizer, Filters, Rotation, Timescale};

    let name = opt_str(cmd, "name").unwrap_or("off");

    let Some(player) = bot.lavalink.get_player_context(guild_id.get()) else {
        return reply(ctx, cmd, "❌ Tidak ada lagu yang diputar").await;
    };

    let filters = match name {
        "bassboost" => Filters {
            equalizer: Some(
                (0..4)
                    .map(|band| Equalizer {
                        band,
                        gain: 0.25,
                    })
                    .collect(),
            ),
            ..Default::default()
        },
        "nightcore" => Filters {
            timescale: Some(Timescale {
                speed: Some(1.2),
                pitch: Some(1.2),
                rate: Some(1.0),
            }),
            ..Default::default()
        },
        "vaporwave" => Filters {
            timescale: Some(Timescale {
                speed: Some(0.8),
                pitch: Some(0.8),
                rate: Some(1.0),
            }),
            ..Default::default()
        },
        "8d" => Filters {
            rotation: Some(Rotation {
                rotation_hz: Some(0.2),
            }),
            ..Default::default()
        },
        _ => Filters::default(),
    };

    match player.set_filters(filters).await {
        Ok(_) => reply(ctx, cmd, format!("🎛️ Filter: **{name}**")).await,
        Err(e) => reply(ctx, cmd, format!("❌ Gagal: {e}")).await,
    }
}

pub async fn playlist((ctx, cmd, bot, guild_id): Args<'_>) {
    let Some(sub) = cmd.data.options.first() else {
        return reply(ctx, cmd, "❌ Pilih subcommand").await;
    };

    match (sub.name.as_str(), &sub.value) {
        ("save", CommandDataOptionValue::SubCommand(opts)) => {
            let name = opts
                .iter()
                .find(|o| o.name == "name")
                .and_then(|o| o.value.as_str())
                .unwrap_or("default")
                .to_lowercase();

            let Some(player) = bot.lavalink.get_player_context(guild_id.get()) else {
                return reply(ctx, cmd, "❌ Tidak ada lagu yang diputar").await;
            };

            let items = match player.get_queue().get_queue().await {
                Ok(q) => q,
                Err(e) => return reply(ctx, cmd, format!("❌ {e}")).await,
            };

            if items.is_empty() {
                return reply(ctx, cmd, "📭 Antrean kosong").await;
            }

            let tracks: Vec<PlaylistTrack> = items
                .iter()
                .map(|t| PlaylistTrack {
                    title: t.track.info.title.clone(),
                    author: t.track.info.author.clone(),
                    uri: t.track.info.uri.clone(),
                    encoded: t.track.encoded.clone(),
                })
                .collect();
            let n = tracks.len();

            bot.shared
                .lock()
                .await
                .playlists
                .entry(guild_id.get())
                .or_default()
                .insert(name.clone(), tracks);

            reply(ctx, cmd, format!("💾 Playlist **{name}** disimpan ({n} lagu)")).await
        }
        ("load", CommandDataOptionValue::SubCommand(opts)) => {
            let name = opts
                .iter()
                .find(|o| o.name == "name")
                .and_then(|o| o.value.as_str())
                .unwrap_or("default")
                .to_lowercase();

            let saved = bot
                .shared
                .lock()
                .await
                .playlists
                .get(&guild_id.get())
                .and_then(|m| m.get(&name))
                .cloned();

            let Some(saved) = saved else {
                return reply(ctx, cmd, format!("❌ Playlist **{name}** tidak ditemukan")).await;
            };

            let player = match ensure_player(ctx, bot, guild_id, cmd.user.id, cmd.channel_id).await {
                Ok(p) => p,
                Err(e) => return reply(ctx, cmd, format!("❌ {e}")).await,
            };

            let n = saved.len();
            let mut tracks = VecDeque::new();
            for t in saved {
                let mut data = TrackData {
                    encoded: t.encoded,
                    info: lavalink_rs::model::track::TrackInfo {
                        title: t.title,
                        author: t.author,
                        uri: t.uri,
                        ..Default::default()
                    },
                    plugin_info: None,
                    user_data: Some(serde_json::json!({ "requester_id": cmd.user.id.get() })),
                };
                data.user_data = Some(serde_json::json!({ "requester_id": cmd.user.id.get() }));
                tracks.push_back(data.into());
            }

            let queue = player.get_queue();
            let empty = queue.get_count().await.unwrap_or(0) == 0;
            let _ = queue.append(tracks);
            if empty {
                let _ = player.skip();
            }

            reply(ctx, cmd, format!("📂 Playlist **{name}** dimuat ({n} lagu)")).await
        }
        ("list", _) => {
            let names = {
                let state = bot.shared.lock().await;
                let mut v: Vec<String> = state
                    .playlists
                    .get(&guild_id.get())
                    .map(|m| {
                        m.iter()
                            .map(|(k, t)| format!("• **{k}** — {} lagu", t.len()))
                            .collect()
                    })
                    .unwrap_or_default();
                v.sort();
                v
            };

            if names.is_empty() {
                reply(ctx, cmd, "📭 Belum ada playlist tersimpan").await
            } else {
                reply(ctx, cmd, format!("📂 Playlist tersimpan:\n{}", names.join("\n"))).await
            }
        }
        _ => {}
    }
}

pub async fn leave((ctx, cmd, bot, guild_id): Args<'_>) {
    let _ = bot.lavalink.delete_player(guild_id.get()).await;
    if let Some(manager) = songbird::get(ctx).await {
        let _ = manager.remove(guild_id).await;
    }
    reply(ctx, cmd, "👋 Keluar dari voice channel").await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffle_keeps_every_elements() {
        let mut q: VecDeque<u32> = (0..20).collect();
        fastrand_shuffle(&mut q);
        let mut sorted: Vec<u32> = q.into_iter().collect();
        sorted.sort();
        assert_eq!(sorted, (0..20).collect::<Vec<u32>>());
    }
}
