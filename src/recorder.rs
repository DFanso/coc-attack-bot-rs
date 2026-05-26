use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::hotkeys;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionKind {
    Click,
    Move,
    Delay { duration: f64 },
    Drag { start_x: i32, start_y: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    #[serde(flatten)]
    pub kind: ActionKind,
    pub x: i32,
    pub y: i32,
    pub timestamp: f64,
    pub relative_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub name: String,
    pub created: String,
    pub duration: f64,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Copy)]
struct Pos {
    x: i32,
    y: i32,
}

pub struct AttackRecorder {
    pub recordings_dir: PathBuf,
    pub auto_detect_clicks: bool,
    is_recording: Arc<AtomicBool>,
    thread: Option<JoinHandle<Vec<Action>>>,
    session_name: Arc<Mutex<String>>,
}

impl AttackRecorder {
    pub fn new(auto_detect_clicks: bool) -> Result<Self> {
        let dir = PathBuf::from("recordings");
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            recordings_dir: dir,
            auto_detect_clicks,
            is_recording: Arc::new(AtomicBool::new(false)),
            thread: None,
            session_name: Arc::new(Mutex::new(String::new())),
        })
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    pub fn start_recording(&mut self, session_name: String) {
        if self.is_recording() {
            tracing::warn!("Already recording a session");
            return;
        }
        *self.session_name.lock().unwrap() = session_name.clone();
        self.is_recording.store(true, Ordering::SeqCst);

        let is_rec = self.is_recording.clone();
        let auto = self.auto_detect_clicks;

        println!("\n=== RECORDING ATTACK SESSION: {session_name} ===");
        if auto {
            println!("Instructions:");
            println!("  1. Perform your attack as normal");
            println!("  2. All clicks are recorded automatically");
            println!("  3. Press F7 to add a delay marker");
            println!("  4. Press F5 to stop, ESC to cancel");
            println!("RECORDING STARTED — Auto-detection enabled");
        } else {
            println!("Press F6 to record each click manually, F5 to stop.");
        }

        let handle = thread::spawn(move || recording_loop(is_rec, auto));
        self.thread = Some(handle);
    }

    pub fn stop_recording(&mut self) -> Option<PathBuf> {
        // Bug fix: don't gate on is_recording(). The recording thread may have
        // already set it to false (e.g. user pressed F5/ESC inside the loop).
        // In that case we still need to join the thread and save whatever
        // actions were captured.
        if self.thread.is_none() {
            tracing::warn!("No recording session active");
            return None;
        }
        self.is_recording.store(false, Ordering::SeqCst);
        let actions = match self.thread.take() {
            Some(h) => h.join().unwrap_or_default(),
            None => Vec::new(),
        };
        if actions.is_empty() {
            println!("No actions recorded");
            return None;
        }
        let name = self.session_name.lock().unwrap().clone();
        match self.save_recording(&name, &actions) {
            Ok(path) => {
                println!("\nRecording saved: {} ({} actions)", path.display(), actions.len());
                Some(path)
            }
            Err(e) => {
                tracing::error!("Failed to save recording: {e}");
                None
            }
        }
    }

    fn save_recording(&self, name: &str, actions: &[Action]) -> Result<PathBuf> {
        let stamp = chrono_like_stamp();
        let filename = format!("{name}_{stamp}.json");
        let path = self.recordings_dir.join(filename);
        let duration = actions.last().map(|a| a.timestamp).unwrap_or(0.0);
        let recording = Recording {
            name: name.into(),
            created: stamp,
            duration,
            actions: actions.to_vec(),
        };
        std::fs::write(&path, serde_json::to_string_pretty(&recording)?)?;
        Ok(path)
    }

    pub fn list_sessions(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.recordings_dir) {
            for e in entries.flatten() {
                let path = e.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        out.push(stem.to_string());
                    }
                }
            }
        }
        out.sort();
        out
    }

    pub fn load_recording(&self, session_name: &str) -> Option<Recording> {
        load_recording_from(&self.recordings_dir, session_name)
    }

    pub fn delete_recording(&self, session_name: &str) -> bool {
        let path = self.recordings_dir.join(format!("{session_name}.json"));
        std::fs::remove_file(&path).is_ok()
    }
}

/// Standalone loader so non-recorder callers (auto-attacker, player) can read sessions
/// without needing an AttackRecorder instance.
pub fn load_recording_from(dir: &std::path::Path, session_name: &str) -> Option<Recording> {
    let path = dir.join(format!("{session_name}.json"));
    if !path.exists() {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}


fn recording_loop(is_rec: Arc<AtomicBool>, auto: bool) -> Vec<Action> {
    let start = Instant::now();
    let mut actions: Vec<Action> = Vec::new();
    let mut last_pos = cursor_position();
    let mut last_click = 0.0_f64;
    let mut prev_left_down = false;
    let mut prev_right_down = false;

    while is_rec.load(Ordering::SeqCst) {
        let now = start.elapsed().as_secs_f64();

        if hotkeys::is_pressed(hotkeys::VK_ESCAPE) {
            println!("\nRecording cancelled");
            is_rec.store(false, Ordering::SeqCst);
            break;
        }
        if hotkeys::is_pressed(hotkeys::VK_F5) {
            println!("\nStopping recording");
            is_rec.store(false, Ordering::SeqCst);
            break;
        }
        if hotkeys::is_pressed(hotkeys::VK_F6) {
            let p = cursor_position();
            actions.push(Action {
                kind: ActionKind::Click,
                x: p.x,
                y: p.y,
                timestamp: now,
                relative_time: now,
            });
            println!("Manual click recorded at ({}, {})", p.x, p.y);
            hotkeys::wait_for_release(hotkeys::VK_F6, 1000);
        }
        if hotkeys::is_pressed(hotkeys::VK_F7) {
            print!("\nEnter delay in seconds: ");
            io::stdout().flush().ok();
            let mut line = String::new();
            io::stdin().lock().read_line(&mut line).ok();
            let dur: f64 = line.trim().parse().unwrap_or(1.0);
            actions.push(Action {
                kind: ActionKind::Delay { duration: dur },
                x: 0,
                y: 0,
                timestamp: now,
                relative_time: now,
            });
            println!("Added {dur}s delay");
            hotkeys::wait_for_release(hotkeys::VK_F7, 1000);
        }

        if auto {
            let left_down = hotkeys::is_pressed(hotkeys::VK_LBUTTON);
            let right_down = hotkeys::is_pressed(hotkeys::VK_RBUTTON);
            // Detect on the press edge to avoid spamming many clicks per single hold.
            let pressed_edge = (left_down && !prev_left_down) || (right_down && !prev_right_down);
            if pressed_edge && (now - last_click) > 0.15 {
                let p = cursor_position();
                actions.push(Action {
                    kind: ActionKind::Click,
                    x: p.x,
                    y: p.y,
                    timestamp: now,
                    relative_time: now,
                });
                println!("Auto-recorded click at ({}, {})", p.x, p.y);
                last_click = now;
            }
            prev_left_down = left_down;
            prev_right_down = right_down;
        }

        // Track significant mouse movements (>50 px)
        let cur = cursor_position();
        if distance(last_pos, cur) > 50.0 {
            actions.push(Action {
                kind: ActionKind::Move,
                x: cur.x,
                y: cur.y,
                timestamp: now,
                relative_time: now,
            });
            last_pos = cur;
        }

        thread::sleep(Duration::from_millis(50));
    }
    actions
}

fn distance(a: Pos, b: Pos) -> f64 {
    let dx = (a.x - b.x) as f64;
    let dy = (a.y - b.y) as f64;
    (dx * dx + dy * dy).sqrt()
}

#[cfg(windows)]
fn cursor_position() -> Pos {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT { x: 0, y: 0 };
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    Pos { x: p.x, y: p.y }
}

#[cfg(not(windows))]
fn cursor_position() -> Pos {
    Pos { x: 0, y: 0 }
}

fn chrono_like_stamp() -> String {
    use time::macros::format_description;
    let t = time::OffsetDateTime::now_local()
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    t.format(format_description!(
        "[year][month][day]_[hour][minute][second]"
    ))
    .unwrap_or_else(|_| "00000000_000000".to_string())
}
