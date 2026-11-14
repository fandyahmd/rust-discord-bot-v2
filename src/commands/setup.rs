//! `/setup` — create the dedicated player channel and post the interactive panel.

use std::sync::Arc;

use serenity::all::{
    ChannelType, CreateChannel, CreateEmbed, EditInteractionResponse, GuildId,
};
use serenity::model::application::CommandInteraction;
use serenity::prelude::Context;

use crate::panel;
use crate::state::BotData;

fn opt_str<'a>(cmd: &'a CommandInteraction, name: &str) -> Option<&'a str> {
    cmd.data
        .options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| o.value.as_str())
}

pub async fn setup(ctx: &Context, cmd: &CommandInteraction, bot: &Arc<BotData>, guild_id: GuildId) {
    let name = opt_str(cmd, "name").unwrap_or("rust-player");

    let channel = match guild_id
        .create_channel(
            &ctx.http,
            CreateChannel::new(name)
                .kind(ChannelType::Text)
                .topic("Ketik judul lagu atau tempel link di sini 🎶"),
        )
        .await
    {
        Ok(c) => c,
        Err(e) => {
            let _ = cmd
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new()
                        .content(format!("❌ Gagal buat channel: {e} (butuh permission Manage Channels)")),
                )
                .await;
            return;
        }
    };

    // Repost the panel in the fresh channel. Old panel (if any) is left alone.
    let posted = panel::post(&bot.lavalink, &bot.shared, &ctx.http, guild_id, channel.id).await;
    if let Err(e) = posted {
        let _ = cmd
            .edit_response(&ctx.http, EditInteractionResponse::new().content(format!("⚠️ {e}")))
            .await;
        return;
    }

    let _ = cmd
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .embed(
                    CreateEmbed::new()
                        .title("✅ Player channel siap")
                        .description(format!(
                            "Channel: <#{}>\n\
                             Ketik judul lagu atau link di sana — panel di atas otomatis di-edit, tidak spam.\n\n\
                             **Tombol:** ⏮️ previous · ⏸️/▶️ pause · ⏭️ next · ⏹️ stop · 🔀 shuffle · 🔊/🔉 volume · 🔁 loop · ♾️ autoplay\n\
                             Join voice channel dulu sebelum request lagu.",
                            channel.id
                        ))
                        .color(0x2ecc71),
                ),
        )
        .await;
}
