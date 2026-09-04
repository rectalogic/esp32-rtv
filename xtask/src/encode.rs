use anyhow::Context;
use clap::{Args, Subcommand};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Args)]
#[command(flatten_help = true)]
pub struct EncodeArgs {
    #[command(subcommand)]
    command: EncodeCommands,
    /// Video output directory
    output_directory: PathBuf,
}

#[derive(Subcommand)]
enum EncodeCommands {
    /// Encode interstitial.mp4 video
    Interstitial,
    /// Encode video
    Video {
        /// Source video to encode
        video_path: PathBuf,
        /// Crop instead of letterbox
        #[arg(short, long, default_value_t = false)]
        crop: bool,
    },
}

pub fn encode(encode_args: &EncodeArgs, workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    match &encode_args.command {
        EncodeCommands::Interstitial => {
            encode_interstitial(&encode_args.output_directory, &workspace_root)
        }
        EncodeCommands::Video { video_path, crop } => encode_video(
            video_path,
            *crop,
            &encode_args.output_directory,
            &workspace_root,
        ),
    }
}

fn encode_video(
    video_path: impl AsRef<Path>,
    crop: bool,
    output_directory: impl AsRef<Path>,
    workspace_root: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let video_path = video_path.as_ref();
    let mut output_path = PathBuf::from(video_path);
    output_path.set_extension("mp4");
    output_path = Path::new(output_directory.as_ref()).join(output_path.file_name().ok_or(
        anyhow::anyhow!("Invalid video path {}", output_path.display()),
    )?);
    let video_filter = if crop {
        "scale=320x240:force_original_aspect_ratio=increase:flags=lanczos,crop=320:240"
    } else {
        "scale=320x240:force_original_aspect_ratio=decrease:reset_sar=1:flags=lanczos,pad=320:240:(ow-iw)/2:(oh-ih)/2"
    };
    let status = Command::new("ffmpeg")
        .current_dir(workspace_root.as_ref())
        .args([
            "-i",
            video_path.to_str().ok_or(anyhow::anyhow!(
                "Invalid video path {}",
                video_path.display()
            ))?,
            "-r",
            "15",
            "-c:v",
            "libx264",
            "-preset",
            "veryslow",
            "-profile:v",
            "baseline",
            "-level",
            "3.0",
            "-c:a",
            "aac",
            "-ar",
            "16000",
            "-ac",
            "1",
            "-vf",
            format!("{video_filter},format=yuv420p").as_str(),
            "-movflags",
            "+faststart",
            "-y",
            output_path.to_str().ok_or(anyhow::anyhow!(
                "Invalid output path {}",
                output_path.display()
            ))?,
        ])
        .status()
        .context("`ffmpeg` failed")?;
    if !status.success() {
        return Err(anyhow::anyhow!("`ffmpeg` failed"));
    }
    Ok(())
}

fn encode_interstitial(
    output_directory: impl AsRef<Path>,
    workspace_root: impl AsRef<Path>,
) -> anyhow::Result<()> {
    // Unique snow/static frames
    const UNIQUE_FRAMES: u32 = 4;
    // Total frames in video - we use "-refs 4" so there is very little overhead for additional frames
    const TOTAL_FRAMES: u32 = UNIQUE_FRAMES * 6;

    let output_path = Path::new(output_directory.as_ref()).join("interstitial.mp4");
    let status = Command::new("ffmpeg")
        .current_dir(workspace_root.as_ref())
        .args([
            "-f", "lavfi",
            "-i", &format!("color=c=0x808080:s=320x240:r=15,noise=alls=100:allf=t+u,eq=contrast=1.4,format=yuv420p,trim=end_frame={UNIQUE_FRAMES},setpts=PTS-STARTPTS,loop=loop=-1:size={UNIQUE_FRAMES}:start=0"),
            "-frames:v", &TOTAL_FRAMES.to_string(),
            "-c:v", "libx264",
            "-preset", "veryslow",
            "-profile:v", "baseline",
            "-level", "3.0",
            "-bf", "0",
            "-refs", &UNIQUE_FRAMES.to_string(),
            "-sc_threshold", "0",
            "-x264-params", "scenecut=0",
            "-crf", "23",
            "-pix_fmt", "yuv420p",
            "-movflags", "+faststart",
            "-y", output_path.to_str().ok_or(anyhow::anyhow!("Invalid output path {}", output_path.display()))?,
        ])
        .status()
        .context("`ffmpeg` failed")?;
    if !status.success() {
        return Err(anyhow::anyhow!("`ffmpeg` failed"));
    }
    Ok(())
}
