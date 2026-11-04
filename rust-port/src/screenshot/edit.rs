//! Shift+Enter: save the copied rectangle where Win+Shift+S saves its
//! screenshots, then open it in Paint to mark up.
use std::path::{Path, PathBuf};

/// Paint's app alias, which opens the file it's given.
const EDITOR: &str = "mspaint.exe";

/// Saves `rgb` (`width` × `height`, red, green, blue, top row first) as a PNG
/// in the Screenshots folder and opens it in Paint, on a thread of its own so
/// encoding a large image doesn't hold up the UI thread.
pub(crate) fn open(rgb: Vec<u8>, width: i32, height: i32) {
    let spawned = std::thread::Builder::new()
        .name("screenshot-edit".into())
        .spawn(move || {
            let Some(folder) = screenshots_folder() else {
                return;
            };
            let path = unused_path(&folder, &file_stem());
            if let Err(error) = save_png(&path, &rgb, width as u32, height as u32) {
                crate::error_log::report("screenshot: saving PNG", error);
                return;
            }
            // Started as the desktop user, so Paint doesn't inherit SWM's
            // elevation.
            if let Err(error) = crate::process::start_program_as_explorer_user(
                EDITOR,
                &[&path.to_string_lossy()],
            ) {
                error.report();
            }
        });
    if let Err(error) = spawned {
        crate::error_log::report("screenshot: starting edit thread", error.to_string());
    }
}

/// The user's Screenshots folder (`Pictures\Screenshots`, wherever it has
/// been moved to), created if it doesn't exist yet.
fn screenshots_folder() -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{
        FOLDERID_Screenshots, KF_FLAG_CREATE, SHGetKnownFolderPath,
    };
    let mut path = std::ptr::null_mut();
    let result = unsafe {
        SHGetKnownFolderPath(
            &FOLDERID_Screenshots,
            KF_FLAG_CREATE as u32,
            std::ptr::null_mut(),
            &mut path,
        )
    };
    let folder = (result >= 0 && !path.is_null()).then(|| {
        let length = (0..).take_while(|&i| unsafe { *path.add(i) } != 0).count();
        let wide = unsafe { std::slice::from_raw_parts(path, length) };
        PathBuf::from(std::ffi::OsString::from_wide(wide))
    });
    // Freed whether or not the call succeeded, as it asks.
    unsafe { CoTaskMemFree(path.cast()) };
    if folder.is_none() {
        crate::error_log::report_win32("SHGetKnownFolderPath(Screenshots)", result as u32);
    }
    folder
}

/// "Screenshot 2026-11-03 162834", the local time, as Win+Shift+S names them.
fn file_stem() -> String {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    let mut now = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut now) };
    format!(
        "Screenshot {:04}-{:02}-{:02} {:02}{:02}{:02}",
        now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond
    )
}

/// `folder\stem.png`, or `stem (2).png` and so on if that is taken.
fn unused_path(folder: &Path, stem: &str) -> PathBuf {
    let candidate = |n: u32| match n {
        1 => folder.join(format!("{stem}.png")),
        n => folder.join(format!("{stem} ({n}).png")),
    };
    (1..)
        .map(candidate)
        .find(|path| !path.exists())
        .expect("some numbered name is free")
}

fn save_png(path: &Path, rgb: &[u8], width: u32, height: u32) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(rgb)
        .map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("swm-screenshot-test-{name}"));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn taken_names_get_a_number() {
        let folder = scratch("names");
        assert_eq!(unused_path(&folder, "Shot"), folder.join("Shot.png"));
        std::fs::write(folder.join("Shot.png"), b"").unwrap();
        std::fs::write(folder.join("Shot (2).png"), b"").unwrap();
        assert_eq!(unused_path(&folder, "Shot"), folder.join("Shot (3).png"));
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn the_png_holds_the_pixels() {
        let folder = scratch("png");
        let path = folder.join("Shot.png");
        // Red, green / blue, white.
        let rgb = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
        save_png(&path, &rgb, 2, 2).unwrap();
        let decoder =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!((info.width, info.height), (2, 2));
        assert_eq!(&pixels[..info.buffer_size()], &rgb);
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn names_follow_win_shift_s() {
        let stem = file_stem();
        assert!(stem.starts_with("Screenshot "), "{stem}");
        // "Screenshot YYYY-MM-DD HHMMSS"
        assert_eq!(stem.len(), "Screenshot 2026-11-03 162834".len(), "{stem}");
    }
}
