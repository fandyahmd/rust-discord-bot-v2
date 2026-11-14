# Rust — Discord Music Bot

Bot musik Discord berbasis **Rust** (Serenity + Songbird + Lavalink v4).
Slash commands, multi-server, dan channel player interaktif di mana member tinggal
mengetik judul lagu atau menempel link — panel dengan tombol di-edit di tempat jadi
channel nggak pernah spam.

## Fitur

- **Putar dari teks/link**: YouTube, YouTube Music, Spotify, SoundCloud, playlist & album.
- **Channel player** (`/setup`): satu channel teks khusus bot; ketik judul lagu di sana
  dan lagu langsung masuk antrean. Satu pesan panel dengan tombol
  ⏮️ prev · ⏸️ pause · ⏭️ next · ⏹️ stop · 🔀 shuffle · 🔊🔉 volume · 🔁 loop · ♾️ autoplay,
  dan panel **di-edit** tiap ganti lagu (tidak spam).
- **Kontrol lengkap**: `/play`, `/pause`, `/resume`, `/skip`, `/stop`, `/nowplaying`,
  `/queue`, `/volume`, `/seek`, `/shuffle`, `/remove`, `/move`, `/clear`, `/leave`, `/ping`.
- **Loop & autoplay**: `/loop off|track|queue`, `/autoplay` (radio otomatis dari lagu terakhir).
- **Filter audio**: `/filter off|bassboost|nightcore|vaporwave|8d`.
- **Playlist tersimpan per server**: `/playlist save|load|list`.
- **Multi-server**: state terpisah per guild (antrean, loop, autoplay, panel, playlist).

## Persyaratan

- Rust 1.75+ (stable)
- Token bot Discord
- Node Lavalink v4. Default-nya memakai node publik
  `lavalinkv4.serenetia.com:443` supaya langsung jalan. Untuk produksi, sebaiknya
  jalankan node Lavalink sendiri.

## Cara pakai

```bash
git clone https://github.com/fandyahmd/rust-discord-bot-v2.git
cd rust-discord-bot-v2

cp .env.example .env
# isi DISCORD_TOKEN dan BOT_USER_ID di .env

cargo build --release
./target/release/rust
```

Bot akan otomatis mendaftarkan slash command (global + per-guild, yang per-guild instan).

### Undang bot

Di Discord Developer Portal, aktifkan **Message Content Intent**
(Bot → Privileged Gateway Intents), lalu undang bot dengan scope
`bot applications.commands` dan permission minimal:

```
View Channels, Send Messages, Manage Messages, Embed Links,
Read Message History, Connect, Speak
```

## Cara pakai di server

1. Undang bot, lalu jalankan `/setup` di channel mana pun (butuh *Manage Channels*).
2. Bot membuat channel `rust-player` beserta panel tombol di dalamnya.
3. Masuk voice channel, lalu ketik judul lagu atau link di channel player.
4. Kontrol lewat tombol panel, atau slash command biasa di channel lain.

## Struktur

```
src/
├── main.rs          # bootstrap, .env loader, slash command sync, event handler
├── commands/        # slash command (mod.rs, music.rs, setup.rs)
├── interactions.rs  # handler pesan channel player + tombol panel
├── panel.rs         # render & edit pesan panel
├── player.rs        # player lifecycle & resolve track
├── hooks.rs         # Lavalink hooks (track start/end, loop, autoplay)
├── state.rs         # state per guild
└── format.rs        # format durasi/progress
lavalink-rs-patched/ # lavalink-rs dengan patch Songbird
```

## Lisensi

MIT
