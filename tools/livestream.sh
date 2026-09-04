#!/usr/bin/env bash

LOG_DIR=$(mktemp -d)
CID_FILE="$LOG_DIR/srs.cid"

cleanup() {
    docker stop $(cat "$CID_FILE")
    wait $PID_DOCKER 2>/dev/null
    kill $PID_TUNNEL $PID_FFMPEG 2>/dev/null
    exit
}
trap cleanup EXIT INT TERM

# Configure tunnel mapped to the url set in LIVESTREAM_URL
# https://developers.cloudflare.com/tunnel/
cloudflared tunnel run --token ${TUNNEL_TOKEN:?Missing cloudflared token} > "$LOG_DIR/tunnel.log" 2>&1 &
PID_TUNNEL=$!

docker run --cidfile "$CID_FILE" --rm -i -p 1935:1935 -p 1985:1985 -p 8080:8080 -e SRS_VHOST_HTTP_REMUX_MOUNT="[vhost]/[app]/[stream].ts" -e SRS_VHOST_HTTP_REMUX_HAS_AUDIO=on -e SRS_VHOST_HTTP_REMUX_HAS_VIDEO=on -e SRS_VHOST_HTTP_REMUX_GUESS_HAS_AV=off ossrs/srs:7 > "$LOG_DIR/srs.log" 2>&1 &
PID_DOCKER=$!

# Wait for docker/srs to come up
# List available devices: ffmpeg -f avfoundation -list_devices true -i ""
sleep 5
ffmpeg -f avfoundation -framerate 30 -video_size 640x480 -i ${FFMPEG_DEVICES:-0:1} -c:a aac -ar 16000 -ac 1 -c:v libx264 -r 15 -g 30 -keyint_min 30 -sc_threshold 0 -x264-params "keyint=30:min-keyint=30:scenecut=0" -preset ultrafast -profile:v baseline -level 3.0 -vf "scale=320x240:flags=bilinear,format=yuv420p" -f flv rtmp://localhost/live/livestream > "$LOG_DIR/ffmpeg.log" 2>&1 &
PID_FFMPEG=$!

tail -f "$LOG_DIR/srs.log" "$LOG_DIR/tunnel.log" "$LOG_DIR/ffmpeg.log"
