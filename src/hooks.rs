//! Lavalink event hooks: now-playing announcement and queue-advance logic.

use lavalink_rs::hook;
use lavalink_rs::model::events;
use lavalink_rs::model::track::{TrackData, TrackLoadData};
use lavalink_rs::prelude::{LavalinkClient, PlayerContext};
use crate::state::{LavaState, LoopMode};

/// Refresh the guild panel when a track starts.
#[hook]
pub async fn track_start(client: LavalinkClient, _session: String, event: &events::TrackStart) {
    let Ok(lava) = client.data::<LavaState>() else {
        return;
    };

    crate::panel::update(
        &client,
        &lava.shared,
        &lava.http,
        serenity::all::GuildId::new(event.guild_id.0),
    )
    .await;
}

/// Repeat / autoplay handling when a track ends naturally.
#[hook]
pub async fn track_end(client: LavalinkClient, _session: String, event: &events::TrackEnd) {
    use events::TrackEndReason;

    if !matches!(
        event.reason,
        TrackEndReason::Finished | TrackEndReason::LoadFailed
    ) {
        return;
    }

    let Some(player) = client.get_player_context(event.guild_id) else {
        return;
    };
    let Ok(lava) = client.data::<LavaState>() else {
        return;
    };

    let settings = lava.shared.lock().await.settings(event.guild_id.0);
    let queue = player.get_queue();

    // Remember the finished track for the ⏮️ button.
    lava.shared
        .lock()
        .await
        .history
        .insert(event.guild_id.0, Some(event.track.clone()));

    match settings.loop_mode {
        LoopMode::Track => {
            let _ = queue.push_to_front(event.track.clone());
            let _ = player.skip();
        }
        LoopMode::Queue => {
            let _ = queue.push_to_back(event.track.clone());
            let _ = player.skip();
        }
        LoopMode::Off => {
            let count = queue.get_count().await.unwrap_or(0);
            if count == 0 && settings.autoplay {
                autoplay_next(&client, &player, &event.track.info).await;
            } else {
                crate::panel::update(
                    &client,
                    &lava.shared,
                    &lava.http,
                    serenity::all::GuildId::new(event.guild_id.0),
                )
                .await;
            }
        }
    }
}

/// Queue a related track when the queue ran dry (YouTube radio fallback).
async fn autoplay_next(
    client: &LavalinkClient,
    player: &PlayerContext,
    last: &lavalink_rs::model::track::TrackInfo,
) {
    let Ok(loaded) = client
        .load_tracks(
            player.guild_id,
            &format!("ytmsearch:{} radio", last.author),
        )
        .await
    else {
        return;
    };

    let Some(TrackLoadData::Search(results)) = loaded.data else {
        return;
    };

    if let Some(track) = take_related(results, &last.identifier) {
        let _ = player.get_queue().push_to_back(track);
        let _ = player.skip();
    }
}

fn take_related(tracks: Vec<TrackData>, skip_id: &str) -> Option<TrackData> {
    tracks.into_iter().find(|t| t.info.identifier != skip_id)
}

/// Drop stale player contexts when the node (re)connects.
#[hook]
pub async fn ready_event(client: LavalinkClient, session_id: String, _event: &events::Ready) {
    println!("[Lavalink] node ready, session={session_id}");
    let _ = client.delete_all_player_contexts().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> TrackData {
        TrackData {
            encoded: String::new(),
            info: lavalink_rs::model::track::TrackInfo {
                identifier: id.to_string(),
                ..Default::default()
            },
            plugin_info: None,
            user_data: None,
        }
    }

    #[test]
    fn autoplay_skips_the_seed_track() {
        let picked = take_related(vec![track("seed"), track("other")], "seed");
        assert_eq!(picked.map(|t| t.info.identifier), Some("other".to_string()));
        assert!(take_related(vec![track("seed")], "seed").is_none());
    }
}
