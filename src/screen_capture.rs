use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::SystemTime;
use xcap::Window;

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

    /// Find the COC / emulator window via title substring match.
    /// Mirrors the Python EnumWindows + lowercase contains check.
    pub fn find_game_window(&mut self) -> Result<Option<&GameWindow>> {
        const NEEDLES: &[&str] = &[
            "clash of clans",
            "google play games",
            "bluestacks",
            "nox",
            "ldplayer",
            "memu",
        ];

        let windows = Window::all().context("Window::all() failed")?;
        for w in windows {
            let title_raw = w.title().to_string();
            let title_lc = title_raw.to_lowercase();
            if NEEDLES.iter().any(|n| title_lc.contains(n)) {
                let x = w.x();
                let y = w.y();
                let width = w.width();
                let height = w.height();

                if width == 0 || height == 0 {
                    continue;
                }
                let gw = GameWindow {
                    title: title_raw,
                    x,
                    y,
                    width,
                    height,
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
    pub fn capture_game_screen(&mut self) -> Result<Option<PathBuf>> {
        if self.last_window.is_none() {
            self.find_game_window()?;
        }

        let Some(gw) = &self.last_window else {
            return Ok(None);
        };

        // Re-resolve the live xcap Window by matching its title — bounds may have moved.
        let target_title = gw.title.clone();
        let windows = Window::all()?;
        let Some(window) = windows.into_iter().find(|w| w.title() == target_title) else {
            tracing::warn!("Game window disappeared between detection and capture");
            self.last_window = None;
            return Ok(None);
        };

        let image = window.capture_image().context("capture_image failed")?;

        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let filename = format!("screenshot_{stamp}.png");
        let filepath = self.screenshot_dir.join(filename);

        image.save(&filepath).context("save PNG failed")?;
        tracing::info!("Screenshot saved: {}", filepath.display());
        Ok(Some(filepath))
    }

    /// Capture the full virtual screen (fallback).
    pub fn capture_full_screen(&self) -> Result<PathBuf> {
        let monitors = xcap::Monitor::all()?;
        let primary = monitors
            .into_iter()
            .find(|m| m.is_primary())
            .or_else(|| xcap::Monitor::all().ok().and_then(|mut v| v.pop()))
            .context("no monitors found")?;

        let image = primary.capture_image()?;
        let stamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let filepath = self.screenshot_dir.join(format!("screenshot_{stamp}.png"));
        image.save(&filepath)?;
        tracing::info!("Full-screen screenshot saved: {}", filepath.display());
        Ok(filepath)
    }
}
