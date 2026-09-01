use crate::{
    input::{ButtonEvent, UserInput},
    littlefs::Littlefs,
    sdcard::SdCard,
    video_player::VideoPlayer,
    videos::find_videos,
    wifi::{SERVICE_NAME, WifiProvisioning},
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::peripherals::Peripherals,
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, EspWifi},
};
use std::thread;
use std::time::Duration;

const INTERSTITIAL: &str = "/littlefs/assets/interstitial.mp4";
const LONG_PRESS_MS: u64 = 500;

pub fn app() -> anyhow::Result<()> {
    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    let video_player = VideoPlayer::new()?;

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    {
        let wifi_prov = WifiProvisioning::new()?;
        if !wifi_prov.is_provisioned()? {
            let message = format!("Provision WiFi for {SERVICE_NAME}");
            video_player.display_text(Some(&message))?;
        }
        wifi_prov.ensure_provisioned(&mut wifi)?;
        video_player.display_text(None)?;
    }

    thread::scope(|scope| -> anyhow::Result<()> {
        let gpio0 = peripherals.pins.gpio0;
        let _user_input_thread = scope.spawn(|| -> anyhow::Result<()> {
            let mut user_input = UserInput::new(gpio0.into()).expect("user input");
            loop {
                match user_input.wait_for_event()? {
                    ButtonEvent::Pressed => {
                        video_player.hilite_video(true)?;
                    }
                    ButtonEvent::Released { held } => {
                        video_player.hilite_video(false)?;
                        if held >= Duration::from_millis(LONG_PRESS_MS) {
                            log::info!("long press, held {held:?}");
                        }
                    }
                }
            }
        });

        let mut interstitial = None;
        let mut videos = None;

        let littlefs = Littlefs::new();
        let sdcard = SdCard::new();

        if littlefs.is_ok() {
            if matches!(std::fs::exists(INTERSTITIAL), Ok(true)) {
                interstitial = Some(INTERSTITIAL);
            }
            if sdcard.is_err() {
                let all_videos = find_videos("/littlefs/videos")?;
                videos = Some(all_videos);
            }
        }

        if sdcard.is_ok() {
            videos = Some(find_videos("/sdcard/rtv")?);
        }

        video_player.play("https://yellowfoot.us-west.host.bsky.network/xrpc/com.atproto.sync.getBlob?did=did%3Aplc%3Azudaqq6xm46a62ogt6izag5c&cid=bafkreigmidpfzfbcxptbqjk5z6c6anfc32ifcjyzrs3y7ajemvli5vsnku#.mp4")?;
        // video_player.play("http://192.168.1.239:8080/live/livestream.ts")?;
        // video_player.play("http://192.168.1.239:8080/live/livestream.m3u8")?;
        // video_player.play("https://stream.place/xrpc/place.stream.playback.getLivePlaylist?streamer=rectalogic.com")?;

        if let Some(mut videos) = videos {
            videos.sort();
            for video in videos.into_iter().cycle() {
                video_player.play(&video)?;
                if let Some(interstitial) = interstitial {
                    video_player.play(interstitial)?;
                }
            }
        }

        Ok(())
    })
}
