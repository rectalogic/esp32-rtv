#!/usr/bin/env bash

ROOT=$(cd -- "$(dirname -- "${BASH_SOURCE[0]:-$0}")" && pwd)/..

docker build -t esp32-rtv "$ROOT/.devcontainer" \
    && docker run --rm -it -e LIVESTREAM_URL \
        -v "$ROOT:/home/esp/esp32-rtv" \
        -v esp32-rtv-cargo:/home/esp/.cargo \
        -w /home/esp/esp32-rtv esp32-rtv \
        "$@"
