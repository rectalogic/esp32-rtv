esp-idf-svc based video player app using ESP board_manager and GMF esp_player component bindings.
Configured for [E32N28P/E32C28P](https://www.lcdwiki.com/2.8inch_ESP32-S3_Display), like [this](https://www.aliexpress.us/item/3256811919417733.html).

# Building

In docker container from .devcontainer/Dockerfile:

```sh-session
$ cargo xtask generate
$ cargo build --release
```

Building a LittleFS filesystem with embedded videos (memory-mapped for
cache-friendly reads during playback):
```sh-session
$ cargo xtask littlefsgen /path/to/videos
```

Flash from host with access to USB:
```sh-session
$ cargo +stable xtask flash firmware
$ cargo +stable xtask flash littlefs
```

The littlefs image (13.5 MB on flash) is written in 2 MiB chunks at 460800 baud
so a dropped USB-Serial-JTAG link only loses one chunk - rerun to finish.

# Development

If anything in `components/e32c28p` is modified, rerun `cargo xtask generate`.

If anything in `components/video_player` is modified, `touch components/video_player/bindings.h`.
