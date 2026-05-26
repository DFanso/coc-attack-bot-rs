use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::SystemTime;
use xcap::Monitor;

#[cfg(windows)]
use windows::Win32::{
    Foundation::{BOOL, HWND, LPARAM, RECT},
    UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
    },
};

pub struct GameWindow {
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub struct ScreenCapture {
    screenshot_dir: PathBuf,
    pub last_window: Option<GameWindow>,
}

impl ScreenCapture {
    pub fn new() -> Result<Self> {
        let dir = PathBuf::from("screenshots");
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            screenshot_dir: dir,
            last_window: None,
        })
    }

    /// Find the COC / emulator window via Win32 EnumWindows + title substring match.
    /// Two-pass: prefer the actual game window; fall back to launcher/emulator windows.
    pub fn find_game_window(&mut self) -> Result<Option<&GameWindow>> {
        const PRIMARY: &[&str] = &["clash of clans"];
        const FALLBACK: &[&str] = &[
            "bluestacks",
            "nox",
            "ldplayer",
            "memu",
            "google play games",
        ];

        let candidates = enum_visible_windows();
        for pass in [PRIMARY, FALLBACK] {
            for (hwnd_raw, title) in &candidates {
                let title_lc = title.to_lowercase();
                if !pass.iter().any(|n| title_lc.contains(n)) {
                    continue;
                }
                let Some((x, y, w, h)) = get_window_rect(*hwnd_raw) else {
                    continue;
                };
                if w == 0 || h == 0 {
                    continue;
                }
                let gw = GameWindow {
                    title: title.clone(),
                    x,
                    y,
                    width: w,
                    height: h,
                };
                tracing::info!(
                    "Found game window: {} at ({}, {}, {}x{})",
                    gw.title,
                    gw.x,
                    gw.y,
                    gw.width,
                    gw.height
                );
                self.last_window = Some(gw);
                return Ok(self.last_window.as_ref());
            }
        }
        tracing::warn!("Could not find COC game window. Make sure the game is running.");
        Ok(None)
    }

    /// Capture the previously-located game window. Re-locates if needed.
    /// Strategy: capture the full monitor via xcap, then crop to the window rect
    /// (re-fetched live so window movement is handled).
    pub fn capture_game_screen(&mut self) -> Result<Option<PathBuf>> {
        if self.last_window.is_none() {
            self.find_game_window()?;
        }
        let Some(gw) = self.last_window.as_ref() else {
            return Ok(None);
        };

        // Re-resolve rect by title so we pick up movement / resizes.
        let target_title_lc = gw.title.to_lowercase();
        let candidates = enum_visible_windows();
        let Some((hwnd_raw, _)) = candidates
            .into_iter()
            .find(|(_, t)| t.to_lowercase() == target_title_lc)
        else {
            tracing::warn!("Game window disappeared between detection and capture");
            self.last_window = None;
            return Ok(None);
        };
        let Some((wx, wy, ww, wh)) = get_window_rect(hwnd_raw) else {
            return Ok(None);
        };

        // Pick the monitor that contains the window's origin (top-left).
        let monitors = Monitor::all().context("Monitor::all() failed")?;
        let monitor = monitors
            .into_iter()
            .find(|m| {
                let mx = m.x();
                let my = m.y();
                let mw = m.width() as i32;
                let mh = m.height() as i32;
                wx >= mx && wx < mx + mw && wy >= my && wy < my + mh
            })
            .or_else(|| {
                Monitor::all()
                    .ok()
                    .and_then(|v| v.into_iter().find(|m| m.is_primary()))
            })
            .context("no matching monitor found")?;

        let mx = monitor.x();
        let my = monitor.y();
        let mw = monitor.width();
        let mh = monitor.height();
        let img = monitor.capture_image().context("capture_image failed")?;

        // Compute crop rect, clamped to monitor bounds.
        let crop_x = (wx - mx).max(0) as u32;
        let crop_y = (wy - my).max(0) as u32;
        let crop_w = ww.min(mw.saturating_sub(crop_x));
        let crop_h = wh.min(mh.saturating_sub(crop_y));
        let cropped = image::imageops::crop_imm(&img, crop_x, crop_y, crop_w, crop_h).to_image();

        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let filepath = self.screenshot_dir.join(format!("screenshot_{stamp}.png"));
        cropped.save(&filepath).context("save PNG failed")?;
        tracing::info!("Screenshot saved: {}", filepath.display());
        Ok(Some(filepath))
    }

    /// Capture the full virtual screen (fallback).
    pub fn capture_full_screen(&self) -> Result<PathBuf> {
        let monitors = Monitor::all()?;
        let primary = monitors
            .into_iter()
            .find(|m| m.is_primary())
            .or_else(|| Monitor::all().ok().and_then(|mut v| v.pop()))
            .context("no monitors found")?;

        let image = primary.capture_image()?;
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let filepath = self
            .screenshot_dir
            .join(format!("screenshot_{stamp}.png"));
        image.save(&filepath)?;
        tracing::info!("Full-screen screenshot saved: {}", filepath.display());
        Ok(filepath)
    }
}

// ─── Win32 helpers ────────────────────────────────────────────────────────

#[cfg(windows)]
fn enum_visible_windows() -> Vec<(isize, String)> {
    let mut data: Vec<(isize, String)> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(enum_proc),
            LPARAM(&mut data as *mut _ as isize),
        );
    }
    data
}

#[cfg(windows)]
unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return BOOL(1);
    }
    let mut buf = vec![0u16; (len as usize) + 1];
    let read = GetWindowTextW(hwnd, &mut buf);
    if read <= 0 {
        return BOOL(1);
    }
    let title = String::from_utf16_lossy(&buf[..read as usize]);
    let data = &mut *(lparam.0 as *mut Vec<(isize, String)>);
    data.push((hwnd.0 as isize, title));
    BOOL(1)
}

#[cfg(windows)]
fn get_window_rect(hwnd_raw: isize) -> Option<(i32, i32, u32, u32)> {
    let hwnd = HWND(hwnd_raw as *mut _);
    let mut rect = RECT::default();
    unsafe {
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return None;
        }
    }
    let w = (rect.right - rect.left).max(0) as u32;
    let h = (rect.bottom - rect.top).max(0) as u32;
    Some((rect.left, rect.top, w, h))
}

#[cfg(not(windows))]
fn enum_visible_windows() -> Vec<(isize, String)> {
    Vec::new()
}

#[cfg(not(windows))]
fn get_window_rect(_hwnd_raw: isize) -> Option<(i32, i32, u32, u32)> {
    None
}
