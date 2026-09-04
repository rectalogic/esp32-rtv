esp-idf-svc based video player app using ESP board_manager and GMF esp_player component bindings.
Configured for [E32N28P/E32C28P](https://www.lcdwiki.com/2.8inch_ESP32-S3_Display), like [this](https://www.aliexpress.us/item/3256811919417733.html).

# Configure WiFi

When the device first boots it displays a "Provision WiFi" message.
Use the ESP BLE Provisioning app to provision: [iOS](https://apps.apple.com/in/app/esp-ble-provisioning/id1473590141) [Android](https://play.google.com/store/apps/details?id=com.espressif.provble)

# Building

In docker container from .devcontainer/Dockerfile:

```sh-session
$ tools/docker.sh cargo xtask bmgr
$ tools/docker.sh cargo build --release
```

Building a LittleFS filesystem with assets and embedded videos:
```sh-session
$ tools/docker.sh cargo xtask embed path/to/videos
```

Flash from host with access to USB:
```sh-session
$ cargo +stable xtask flash firmware
$ cargo +stable xtask flash embed
```

# Encoding Video

To rebuild `assets/interstitial.mp4` static/snow video:
```sh-session
$ cargo +stable xtask encode assets/ interstitial
```

Encode `my-video.avi` to `/path/to/videos/my-video.mp4`
(either in container or on host)
```sh-session
$ tools/docker.sh cargo xtask encode path/to/videos video my-video.avi
$ cargo +stable xtask encode path/to/videos video my-video.avi
```

# Live Streaming Video

Setup a [Cloudflare Tunnel](https://developers.cloudflare.com/tunnel/) routing to `http://localhost:8080/live/livestream.ts`.

Ensure `ffmpeg`, `cloudflared` and OrbStack or Docker are installed:
`brew install ffmpeg cloudflared orbstack`

Build firmware where XXX is your cloudflare tunnel domain name:
```sh-session
$ LIVESTREAM_URL=https://XXX/live/livestream.ts tools/docker.sh cargo build --release
```
Run `TUNNEL_TOKEN=XXX tools/livestream.sh` where XXX is your cloudflare tunnel token.

# Development

If anything in `components/e32c28p` is modified, rerun `cargo xtask bmgr`.

If anything in `components/video_player` is modified, `touch components/video_player/bindings.h`.
