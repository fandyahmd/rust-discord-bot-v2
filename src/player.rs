//! Voice connection + track resolution helpers.

use std::sync::Arc;

use lavalink_rs::model::track::{TrackData, TrackLoadData};
use lavalink_rs::model::GuildId;
use serenity::all::{ChannelId, GuildId as SGuildId, Http, UserId};
use serenity::prelude::Context;

use crate::state::BotData;

/// Data attached to each player so hooks know where to post.
pub type PlayerChannel = (ChannelId, Arc<Http>);

/// Join the user's voice channel (if needed) and return a ready player.
pub async fn ensure_player(
    ctx: &Context,
    bot: &BotData,
    guild_id: SGuildId,
    user_id: UserId,
    channel_id: ChannelId,
) -> Result<lavalink_rs::prelude::PlayerContext, String> {
    let guild = GuildId(guild_id.get());

    if let Some(player) = bot.lavalink.get_player_context(guild) {
        return Ok(player);
    }

    let manager = songbird::get(ctx)
        .await
        .ok_or("Voice manager belum siap")?
        .clone();

    let target_vc = ctx
        .cache
        .guild(guild_id)
        .and_then(|g| g.voice_states.get(&user_id).and_then(|vs| vs.channel_id))
        .ok_or("Masuk ke Voice Channel dulu ya")?;

    let (connection_info, _) = manager
        .join_gateway(guild_id, target_vc)
        .await
        .map_err(|e| format!("Gagal join voice: {e}"))?;

    let mut conn: lavalink_rs::model::player::ConnectionInfo = connection_info.into();
    conn.channel_id = Some(target_vc.into());

    bot.lavalink
        .create_player_context_with_data::<PlayerChannel>(
            guild,
            conn,
            Arc::new((channel_id, ctx.http.clone())),
        )
        .await
        .map_err(|e| format!("Gagal inisialisasi Lavalink: {e}"))
}

/// Resolve a query into tracks. Returns `(tracks, playlist_name)`.
pub async fn resolve(
    bot: &BotData,
    guild_id: SGuildId,
    query: &str,
) -> Result<(Vec<TrackData>, Option<String>), String> {
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
