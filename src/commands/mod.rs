//! Slash command definitions and dispatch.

use std::sync::Arc;

use serenity::all::{CommandOptionType, CreateCommand, CreateCommandOption};
use serenity::model::application::CommandInteraction;
use serenity::prelude::Context;

use crate::state::BotData;

pub mod music;
pub mod setup;

pub fn definitions() -> Vec<CreateCommand> {
    vec![
        CreateCommand::new("play")
            .description("Putar lagu atau playlist")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "query",
                    "Judul lagu atau link (YouTube/Spotify/SoundCloud)",
                )
                .required(true)
                .set_autocomplete(true),
            ),
        CreateCommand::new("pause").description("Jeda lagu"),
        CreateCommand::new("resume").description("Lanjutkan lagu"),
        CreateCommand::new("skip").description("Lewati lagu sekarang"),
        CreateCommand::new("stop").description("Stop dan bersihkan antrean"),
        CreateCommand::new("queue")
            .description("Lihat antrean")
            .add_option(CreateCommandOption::new(
                CommandOptionType::Integer,
                "page",
                "Halaman (default 1)",
            )),
        CreateCommand::new("nowplaying").description("Info lagu yang sedang diputar"),
        CreateCommand::new("volume")
            .description("Atur volume (0-200)")
            .add_option(
                CreateCommandOption::new(CommandOptionType::Integer, "level", "Volume 0-200")
                    .required(true)
                    .min_int_value(0)
                    .max_int_value(200),
            ),
        CreateCommand::new("seek")
            .description("Lompat ke detik tertentu")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "position",
                    "Contoh: 90, 1:30, atau 1m30s",
                )
                .required(true),
            ),
        CreateCommand::new("loop")
            .description("Mode pengulangan")
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "mode", "off / track / queue")
                    .add_string_choice("off", "off")
                    .add_string_choice("track", "track")
                    .add_string_choice("queue", "queue"),
            ),
        CreateCommand::new("shuffle").description("Acak antrean"),
        CreateCommand::new("remove")
            .description("Hapus lagu dari antrean")
            .add_option(
                CreateCommandOption::new(CommandOptionType::Integer, "index", "Nomor antrean")
                    .required(true)
                    .min_int_value(1),
            ),
        CreateCommand::new("move")
            .description("Pindahkan posisi lagu di antrean")
            .add_option(
                CreateCommandOption::new(CommandOptionType::Integer, "from", "Dari nomor")
                    .required(true)
                    .min_int_value(1),
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::Integer, "to", "Ke nomor")
                    .required(true)
                    .min_int_value(1),
            ),
        CreateCommand::new("clear").description("Kosongkan antrean (lagu aktif tetap jalan)"),
        CreateCommand::new("autoplay")
            .description("Putar lagu terkait otomatis saat antrean habis")
            .add_option(
                CreateCommandOption::new(CommandOptionType::Boolean, "enabled", "Aktif/nonaktif")
                    .required(true),
            ),
        CreateCommand::new("filter")
            .description("Efek audio")
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "name", "Nama filter")
                    .required(true)
                    .add_string_choice("off", "off")
                    .add_string_choice("bassboost", "bassboost")
                    .add_string_choice("nightcore", "nightcore")
                    .add_string_choice("vaporwave", "vaporwave")
                    .add_string_choice("8d", "8d"),
            ),
        CreateCommand::new("playlist")
            .description("Simpan atau muat playlist")
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "save", "Simpan antrean")
                    .add_sub_option(
                        CreateCommandOption::new(CommandOptionType::String, "name", "Nama playlist")
                            .required(true),
                    ),
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::SubCommand, "load", "Muat playlist")
                    .add_sub_option(
                        CreateCommandOption::new(CommandOptionType::String, "name", "Nama playlist")
                            .required(true),
                    ),
            )
            .add_option(CreateCommandOption::new(
                CommandOptionType::SubCommand,
                "list",
                "Lihat playlist tersimpan",
            )),
        CreateCommand::new("setup")
            .description("Buat channel player khusus bot (ketik lagu di sana)")
            .default_member_permissions(serenity::all::Permissions::MANAGE_CHANNELS)
            .add_option(CreateCommandOption::new(
                CommandOptionType::String,
                "name",
                "Nama channel (default: rust-player)",
            )),
        CreateCommand::new("leave").description("Keluarkan bot dari voice channel"),
        CreateCommand::new("ping").description("Cek latensi"),
    ]
}

pub async fn handle(ctx: Context, cmd: CommandInteraction, bot: Arc<BotData>) {
    let Some(guild_id) = cmd.guild_id else {
        let _ = cmd
            .create_response(
                &ctx.http,
                serenity::all::CreateInteractionResponse::Message(
                    serenity::all::CreateInteractionResponseMessage::new()
                        .content("❌ Command musik hanya bisa dipakai di server"),
                ),
            )
            .await;
        return;
    };

    let _ = cmd.defer(&ctx.http).await;
    let args = (&ctx, &cmd, &bot, guild_id);

    match cmd.data.name.as_str() {
        "play" => music::play(args).await,
        "pause" => music::pause_resume(args, true).await,
        "resume" => music::pause_resume(args, false).await,
        "skip" => music::skip(args).await,
        "stop" => music::stop(args).await,
        "queue" => music::queue(args).await,
        "nowplaying" => music::now_playing(args).await,
        "volume" => music::volume(args).await,
        "seek" => music::seek(args).await,
        "loop" => music::loop_mode(args).await,
        "shuffle" => music::shuffle(args).await,
        "remove" => music::remove(args).await,
        "move" => music::move_track(args).await,
        "clear" => music::clear(args).await,
        "autoplay" => music::autoplay(args).await,
        "filter" => music::filter(args).await,
        "playlist" => music::playlist(args).await,
        "setup" => setup::setup(&ctx, &cmd, &bot, guild_id).await,
        "leave" => music::leave(args).await,
        "ping" => {
            let latency = bot
                .shard_manager
                .lock()
                .await
                .as_ref()
                .map(|m| m.runners.clone());
            let ms = match latency {
                Some(runners) => {
                    let runners = runners.lock().await;
                    runners
                        .values()
                        .next()
                        .and_then(|r| r.latency)
                        .map(|d| format!("{} ms", d.as_millis()))
                        .unwrap_or_else(|| "—".into())
                }
                None => "—".into(),
            };
            music::reply(&ctx, &cmd, format!("🏓 Pong! Gateway `{ms}`")).await;
        }
        _ => {}
    }
}
