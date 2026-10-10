//! SWM owns file-type routing and external-tool policy; NFM executes and renders plans.
use nfm_egui::Cancellation;
pub use nfm_preview_file::resolved::{
    CommandOutput, CommandSpec, PreviewPlan, ResolvedPreviewProvider,
};
use nfm_preview_file::text::{SyntaxChoice, TextContent, TextPreviewOptions, TextPreviewRequest};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

pub type FilePreviewResolver =
    Arc<dyn Fn(&Path, Cancellation) -> Result<Option<PreviewPlan>, String> + Send + Sync>;

/// These settings belong to the host. Replace `Configuration.file_preview_resolver`
/// with any custom closure to provide different routing or commands.
#[derive(Clone, Debug)]
pub struct MediaPreviewSettings {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub video_position: f64,
    pub files: TextPreviewOptions,
}
impl Default for MediaPreviewSettings {
    fn default() -> Self {
        Self {
            ffmpeg: "ffmpeg.exe".into(),
            ffprobe: "ffprobe.exe".into(),
            video_position: 0.25,
            files: TextPreviewOptions {
                ..Default::default()
            },
        }
    }
}
pub fn resolver(settings: MediaPreviewSettings) -> FilePreviewResolver {
    Arc::new(move |path, cancel| {
        if cancel.is_cancelled() {
            return Err("preview cancelled".into());
        }
        settings.files.validate()?;
        let metadata = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let video = matches!(
            extension.as_str(),
            "mp4" | "mkv" | "mov" | "avi" | "webm" | "m4v" | "wmv"
        );
        let image = matches!(
            extension.as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tif" | "tiff"
        );
        if metadata.is_dir() || !(image || video) {
            return Ok(Some(PreviewPlan::Text {
                request: {
                    let path = path.to_owned();
                    let name = path.to_string_lossy().into_owned();
                    let mut request = TextPreviewRequest::new(Arc::new(
                        move |max_bytes: usize, cancel: &dyn Fn() -> bool| {
                            read_content(&path, max_bytes, cancel)
                        },
                    ));
                    request.name = Some(name);
                    request
                },
                options: settings.files.clone(),
            }));
        }
        let mut command = CommandSpec::new(&settings.ffmpeg);
        command.arguments = vec!["-loglevel".into(), "error".into()];
        if video {
            if !settings.video_position.is_finite()
                || !(0.0..=1.0).contains(&settings.video_position)
            {
                return Err("video_position must be between 0 and 1".into());
            }
            let mut probe = CommandSpec::new(&settings.ffprobe);
            probe.arguments = vec![
                "-v".into(),
                "error".into(),
                "-show_entries".into(),
                "format=duration".into(),
                "-of".into(),
                "default=noprint_wrappers=1:nokey=1".into(),
                path.to_string_lossy().into_owned(),
            ];
            probe.output_limit = 64 * 1024;
            let result =
                nfm_preview_file::resolved::run(&probe, || cancel.is_cancelled(), |_, _| true)?;
            if !result.status.success() || result.truncated {
                return Err(format!(
                    "ffprobe failed: {}",
                    String::from_utf8_lossy(&result.stderr).trim()
                ));
            }
            let duration = String::from_utf8_lossy(&result.stdout)
                .trim()
                .parse::<f64>()
                .map_err(|_| "ffprobe returned an invalid duration")?;
            if !duration.is_finite() || duration < 0.0 {
                return Err("ffprobe returned an invalid duration".into());
            }
            command.arguments.extend([
                "-ss".into(),
                format!("{:.3}", duration * settings.video_position),
            ]);
        }
        command.arguments.extend([
            "-i".into(),
            path.to_string_lossy().into_owned(),
            "-frames:v".into(),
            "1".into(),
            "-vf".into(),
            "scale=1280:1280:force_original_aspect_ratio=decrease".into(),
            "-f".into(),
            "image2pipe".into(),
            "-vcodec".into(),
            "png".into(),
            "pipe:1".into(),
        ]);
        command.output_limit = settings.files.max_bytes;
        Ok(Some(PreviewPlan::Command {
            command,
            output: CommandOutput::Image,
            center_line: None,
            max_scrollback_lines: 8192,
            success_exit_codes: vec![0],
        }))
    })
}

// SWM supplies bounded file, directory, and binary content to NFM.
fn cancelled(cancel: &dyn Fn() -> bool) -> Result<(), String> {
    if cancel() {
        Err("File preview cancelled".into())
    } else {
        Ok(())
    }
}
fn read_content(
    path: &Path,
    max_bytes: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<TextContent, String> {
    cancelled(cancel)?;
    let metadata = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut plain = metadata.is_dir();
    let text = if metadata.is_dir() {
        let mut entries = Vec::new();
        let mut bytes = 0usize;
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            cancelled(cancel)?;
            let entry = entry.map_err(|e| e.to_string())?;
            let name = format!(
                "{}{}",
                entry.file_name().to_string_lossy(),
                if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                    "/"
                } else {
                    ""
                }
            );
            bytes += name.len() + 1;
            if bytes > max_bytes {
                return Err("Directory preview exceeds max_bytes".into());
            }
            entries.push(name);
        }
        entries.sort();
        entries.join("\n")
    } else {
        if !metadata.is_file() {
            return Err("File preview requires a regular file or directory".into());
        }
        if metadata.len() > max_bytes as u64 {
            return Err("File preview exceeds max_bytes".into());
        }
        let mut bytes = Vec::new();
        fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(max_bytes as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        cancelled(cancel)?;
        if bytes.len() > max_bytes {
            return Err("File preview exceeds max_bytes".into());
        }
        if bytes.contains(&0) || std::str::from_utf8(&bytes).is_err() {
            plain = true;
            format!("Binary file\n{}\n{} bytes", path.display(), metadata.len())
        } else {
            String::from_utf8(bytes)
                .map_err(|e| e.to_string())?
                .trim_start_matches('\u{feff}')
                .to_owned()
        }
    };
    Ok(TextContent {
        text,
        syntax: plain.then_some(SyntaxChoice::PlainText),
    })
}
