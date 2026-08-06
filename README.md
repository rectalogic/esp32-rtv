esp-idf-svc based video player app using ESP board_manager and GMF esp_player component bindings.
Configured for [E32N28P/E32C28P](https://www.lcdwiki.com/2.8inch_ESP32-S3_Display), like [this](https://www.aliexpress.us/item/3256811919417733.html).

# Building

In docker container from .devcontainer/Dockerfile:

```sh-session
$ cargo xtask bmgr
$ cargo build --release
```

Building a LittleFS filesystem with embedded videos:
```sh-session
$ cargo xtask littlefs /path/to/videos
```

Flash from host with access to USB:
```sh-session
$ cargo +stable xtask flash firmware
$ cargo +stable xtask flash littlefs
```

# Development

If anything in `components/e32c28p` is modified, rerun `cargo xtask generate`.

If anything in `components/video_player` is modified, `touch components/video_player/bindings.h`.
