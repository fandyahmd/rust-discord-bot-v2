//! Voice connection + track resolution helpers.

use std::sync::Arc;

use lavalink_rs::model::track::{TrackData, TrackLoadData};
use lavalink_rs::model::GuildId;
use serenity::all::{ChannelId, GuildId as SGuildId, Http, UserId};
use serenity::prelude::Context;

use crate::state::BotData;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_target_checks_user_and_bot_channels() {
        let channel = ChannelId::new(123);
        let other = ChannelId::new(456);
        assert!(voice_target(None, None).is_err());
        assert!(voice_target(None, Some(channel)).is_err());
        assert_eq!(voice_target(Some(channel), None).unwrap(), channel);
        assert_eq!(voice_target(Some(channel), Some(channel)).unwrap(), channel);
        assert!(voice_target(Some(channel), Some(other)).is_err());
    }
}

/// Data attached to each player so hooks know where to post.
pub type PlayerChannel = (ChannelId, Arc<Http>);

fn voice_target(user: Option<ChannelId>, bot: Option<ChannelId>) -> Result<ChannelId, String> {
    let user = user.ok_or("Masuk ke Voice Channel dulu ya")?;
    if bot.is_some_and(|channel| channel != user) {
        return Err("Kamu harus berada di Voice Channel yang sama dengan bot".into());
    }
    Ok(user)
}

/// Join the user's voice channel (if needed) and return a ready player.
pub async fn ensure_player(
    ctx: &Context,
    bot: &BotData,
    guild_id: SGuildId,
    user_id: UserId,
    channel_id: ChannelId,
) -> Result<lavalink_rs::prelude::PlayerContext, String> {
    let guild = GuildId(guild_id.get());

    let (user_vc, bot_vc) = {
        let guild = ctx.cache.guild(guild_id).ok_or("Data server belum siap")?;
        let channel = |id| guild.voice_states.get(&id).and_then(|vs| vs.channel_id);
        (channel(user_id), channel(ctx.cache.current_user().id))
    };
    let target_vc = voice_target(user_vc, bot_vc)?;

    let manager = songbird::get(ctx)
        .await
        .ok_or("Voice manager belum siap")?
        .clone();
    let gateway_ready = if let Some(call) = manager.get(guild_id) {
        call.lock().await.current_connection().is_some()
    } else {
        false
    };
    let existing = bot.lavalink.get_player_context(guild);
    if bot_vc == Some(target_vc) && gateway_ready {
        if let Some(player) = existing.as_ref() {
            if player.get_player().await.is_ok_and(|p| p.state.connected) {
                return Ok(player.clone());
            }
        }
    }

    let (connection_info, _) = manager
        .join_gateway(guild_id, target_vc)
        .await
        .map_err(|e| format!("Gagal join voice: {e}"))?;

    let mut conn: lavalink_rs::model::player::ConnectionInfo = connection_info.into();
    conn.channel_id = Some(target_vc.into());

    let player = if let Some(player) = existing {
        player
            .update_player(&lavalink_rs::model::http::UpdatePlayer {
                voice: Some(conn),
                ..Default::default()
            }, false)
            .await
            .map_err(|e| format!("Gagal menghubungkan ulang voice: {e}"))?;
        player
    } else {
        bot.lavalink
            .create_player_context_with_data::<PlayerChannel>(
                guild,
                conn,
                Arc::new((channel_id, ctx.http.clone())),
            )
            .await
            .map_err(|e| format!("Gagal inisialisasi Lavalink: {e}"))?
    };

    // Wait for Lavalink's voice handshake before accepting a play request.
    for _ in 0..20 {
        if player.get_player().await.is_ok_and(|p| p.state.connected) {
            return Ok(player);
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    Err("Koneksi voice belum siap. Coba /play lagi setelah bot terhubung".into())
}

/// Resolve a query into tracks. Returns `(tracks, playlist_name)`.
pub async fn resolve(
    bot: &BotData,
    guild_id: SGuildId,
    query: &str,
) -> Result<(Vec<TrackData>, Option<String>), String> {
    if let Some(track) = bot.suggestions.track(query).await {
        eprintln!("[play] guild={} using cached autocomplete track", guild_id);
        return Ok((vec![track], None));
    }
    let identifier = if query.starts_with("http://") || query.starts_with("https://") {
        query.to_string()
    } else {
        format!("ytsearch:{query}")
    };

    let loaded = bot
        .lavalink
        .load_tracks(GuildId(guild_id.get()), &identifier)
        .await
        .map_err(|e| format!("Pencarian gagal: {e}"))?;

    match loaded.data {
        Some(TrackLoadData::Track(t)) => Ok((vec![t], None)),
        Some(TrackLoadData::Search(mut v)) if !v.is_empty() => Ok((vec![v.remove(0)], None)),
        Some(TrackLoadData::Search(_)) => Err("Lagu tidak ditemukan".into()),
        Some(TrackLoadData::Playlist(p)) => Ok((p.tracks, Some(p.info.name))),
        Some(TrackLoadData::Error(e)) => Err(format!("Sumber error: {}", e.message)),
        None => Err("Tidak ada hasil dari Lavalink".into()),
    }
}

/// Attach the requester id so the now-playing embed can mention them.
pub fn tag_requester(tracks: &mut [TrackData], user_id: u64) {
    let data = serde_json::json!({ "requester_id": user_id });
    for t in tracks.iter_mut() {
        t.user_data = Some(data.clone());
    }
}
