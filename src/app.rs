use crate::{
    input::UserInput, littlefs::Littlefs, sdcard::SdCard, video_player::VideoPlayer,
    videos::find_videos,
};
use esp_idf_svc::hal::{delay::FreeRtos, peripherals::Peripherals};
use std::thread;

const INTERSTITIAL: &str = "/littlefs/interstitial.mp4";

pub fn app() -> anyhow::Result<()> {
    let peripherals = Peripherals::take()?;
    let video_player = VideoPlayer::new()?;

    thread::scope(|scope| {
        let gpio0 = peripherals.pins.gpio0;
        let _user_input_thread = scope.spawn(|| {
            let mut user_input = UserInput::new(gpio0.into()).expect("user input");
            loop {
                user_input.wait_for_input();
                video_player.invert_display(false);
                FreeRtos::delay_ms(250);
                video_player.invert_display(true);
            }
        });

        let mut interstitial = None;
        let mut videos = None;

        let sdcard = SdCard::new();
        let littlefs = Littlefs::new();

        if littlefs.is_ok() {
            if matches!(std::fs::exists(INTERSTITIAL), Ok(true)) {
                interstitial = Some(INTERSTITIAL);
            }
            if sdcard.is_err() {
                let mut all_videos = find_videos("/littlefs")?;
                if interstitial.is_some() {
                    all_videos.retain(|f| f != INTERSTITIAL);
                }
                videos = Some(all_videos);
            }
        }

        if sdcard.is_ok() {
            videos = Some(find_videos("/sdcard/rtv")?);
        }

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
