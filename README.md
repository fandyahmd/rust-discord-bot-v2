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

## Health endpoint

Bot menjalankan HTTP server kecil di `PORT` (default **8080**) yang dipakai platform
PaaS untuk liveness probe — ini yang membuat **Web Service tidak dianggap idle**
sehingga tidak di-sleep:

```bash
curl http://localhost:8080/healthz
# 503 {"status":"starting","uptime_seconds":2}   ← saat boot
# 200 {"status":"ok","uptime_seconds":42}         ← gateway + Lavalink siap
```

Path apa pun dijawab (probe tidak peduli path), tanpa dependency web framework.
Ganti port lewat env `PORT`.

## Deploy via Docker

Image-nya kecil (~150 MB) dan hanya butuh dua environment variable.

```bash
docker build -t rust-bot .
docker run -d --name rust-bot --restart unless-stopped \
  -p 8080:8080 \
  -e DISCORD_TOKEN=your_token \
  -e BOT_USER_ID=123456789012345678 \
  rust-bot
```

Opsional: tambahkan `-e LAVALINK_HOST=... -e LAVALINK_PASSWORD=... -e LAVALINK_SSL=true`
kalau memakai node Lavalink sendiri, dan `-e PORT=...` untuk ganti port health check.

### Koyeb

1. Push repo ini ke GitHub (sudah tersedia di `fandyahmd/rust-discord-bot-v2`).
2. Di Koyeb: **Create Service → GitHub → repo ini**.
3. Builder: **Dockerfile** (otomatis terdeteksi).
4. Instance: **Free / Nano** sudah cukup (bot idle ~50 MB RAM).
5. **Ports**: expose **HTTP**, port **8080** (bot menyediakan `/healthz`).
6. **Environment variables**:
   - `DISCORD_TOKEN` = token bot
   - `BOT_USER_ID` = Application ID
7. **Health check**: HTTP, path `/healthz`, port `8080`. Ini mencegah instance
   dianggap idle lalu di-scale-to-zero/di-sleep.
8. Deploy.

### Render

1. **New → Web Service** (boleh; bot sudah listen HTTP di `PORT`).
2. Connect repo GitHub ini, language **Docker**.
3. **Health Check Path**: `/healthz`
4. Environment variables: `DISCORD_TOKEN`, `BOT_USER_ID`
   (Render mengisi `PORT` otomatis — jangan di-hardcode).
5. Plan Free/Starter, lalu **Create Web Service**.

> Render Free Web Service spin-down setelah idle ~15 menit. Karena ada health check
> ke `/healthz` dan bot aktif menerima probe, service tetap terjaga. Kalau tetap
> ingin tanpa risiko sleep, pakai **Background Worker** (tanpa port) — Dockerfile
> yang sama tetap jalan.

Catatan: health check otomatis (`HEALTHCHECK` di Dockerfile) aktif dan mengenai
`127.0.0.1:8080`; kalau `PORT` diganti, `HEALTHCHECK` tetap benar hanya di port 8080
— override dengan `-e PORT=8080` (default).

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
