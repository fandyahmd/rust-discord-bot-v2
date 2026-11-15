use std::sync::Arc;
use std::time::Instant;

use serenity::async_trait;
use serenity::http::Http;
use serenity::model::application::{Command, Interaction};
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use songbird::SerenityInit;

use rust::state::{BotData, LavaState, SharedState};
use rust::health::Health;
use rust::{commands, hooks, interactions};

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("[Rust] logged in as {} ({})", ready.user.name, ready.user.id);

        let defs = commands::definitions();
        let count = defs.len();

        // Global commands take up to an hour to propagate; guild-scoped ones are instant,
        // so register both. Guild commands shadow the global copy inside that guild.
        match Command::set_global_commands(&ctx.http, defs.clone()).await {
            Ok(_) => println!("[Rust] {count} global slash commands synced"),
            Err(e) => eprintln!("[Rust] global command sync failed: {e:?}"),
        }

        for guild in &ready.guilds {
            match guild.id.set_commands(&ctx.http, defs.clone()).await {
                Ok(_) => println!("[Rust] {count} slash commands synced to guild {}", guild.id),
                Err(e) => eprintln!("[Rust] guild {} sync failed: {e:?}", guild.id),
            }
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let bot = {
            let data = ctx.data.read().await;
            match data.get::<BotData>() {
                Some(b) => b.clone(),
                None => return,
            }
        };

        match interaction {
            Interaction::Command(cmd) => commands::handle(ctx, cmd, bot).await,
            Interaction::Component(cmp) => interactions::on_component(&ctx, &cmp, &bot).await,
            Interaction::Autocomplete(ac) => {
                rust::autocomplete::on_autocomplete(&ctx, &ac, &bot).await
            }
            _ => {}
        }
    }

    async fn message(&self, ctx: Context, msg: serenity::model::channel::Message) {
        let bot = {
            let data = ctx.data.read().await;
            match data.get::<BotData>() {
                Some(b) => b.clone(),
                None => return,
            }
        };
        interactions::on_message(&ctx, &msg, &bot).await;
    }
}

/// Minimal `.env` loader: `KEY=VALUE` lines, `#` comments, optional quotes.
/// Real environment variables win over the file.
fn load_dotenv() {
    let Ok(text) = std::fs::read_to_string(".env") else {
        return;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if !key.is_empty() && std::env::var_os(key).is_none() {
            std::env::set_var(key, value);
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() {
    load_dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,lavalink_rs=info".into()),
        )
        .init();

    let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN wajib di-set (lihat .env.example)");
    let bot_user_id: u64 = env_or("BOT_USER_ID", "")
        .parse()
        .expect("BOT_USER_ID wajib di-set = Application ID bot (lihat .env.example)");

    let shared = Arc::new(Mutex::new(SharedState::default()));
    // Separate handle so Lavalink hooks can edit the panel without a serenity Context.
    let http = Arc::new(Http::new(&token));

    // PaaS web services sleep when nothing listens on the port, so always serve one.
    let health = Health::new(Instant::now());
    let port: u16 = env_or("PORT", "8080").parse().unwrap_or(8080);
    {
        let health = health.clone();
        tokio::spawn(async move { rust::health::serve(health, port).await });
    }

    let events = lavalink_rs::model::events::Events {
        ready: Some(hooks::ready_event),
        track_start: Some(hooks::track_start),
        track_end: Some(hooks::track_end),
        ..Default::default()
    };

    // Public Lavalink node (Serenetia) is the default so the bot works out of the box.
    let node = lavalink_rs::prelude::NodeBuilder {
        hostname: env_or("LAVALINK_HOST", "lavalinkv4.serenetia.com:443"),
        is_ssl: env_or("LAVALINK_SSL", "true") != "false",
        events: lavalink_rs::model::events::Events::default(),
        password: env_or("LAVALINK_PASSWORD", "https://dsc.gg/ajidevserver"),
        user_id: bot_user_id.into(),
        session_id: None,
    };

    println!("[Rust] connecting to Lavalink...");
    let lavalink = lavalink_rs::prelude::LavalinkClient::new_with_data(
        events,
        vec![node],
        lavalink_rs::model::client::NodeDistributionStrategy::round_robin(),
        Arc::new(LavaState {
            shared: shared.clone(),
            http: http.clone(),
            health: Some(health.clone()),
        }),
    )
    .await;

    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .register_songbird()
        .await
        .expect("gagal membuat Discord client");

    {
        let mut data = client.data.write().await;
        data.insert::<BotData>(Arc::new(BotData {
            start_time: Instant::now(),
            health: health.clone(),
            suggestions: rust::autocomplete::SuggestionCache::default(),
            shard_manager: Mutex::new(None),
            lavalink,
            shared,
        }));
    }

    if let Err(e) = client.start().await {
        eprintln!("[Rust] client error: {e:?}");
    }
}
