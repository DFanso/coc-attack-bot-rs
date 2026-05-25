use anyhow::Result;
use enigo::{Button, Coordinate, Direction, Enigo, Mouse, Settings};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::hotkeys;
use crate::recorder::{load_recording_from, Action, ActionKind, Recording};

pub struct AttackPlayer {
    pub recordings_dir: PathBuf,
    is_playing: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub playback_speed: f64,
}

impl AttackPlayer {
    pub fn new<P: Into<PathBuf>>(recordings_dir: P) -> Self {
        Self {
            recordings_dir: recordings_dir.into(),
            is_playing: Arc::new(AtomicBool::new(false)),
            thread: None,
            playback_speed: 1.0,
        }
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::SeqCst)
    }

    pub fn set_playback_speed(&mut self, speed: f64) {
        if speed > 0.0 {
            self.playback_speed = speed;
            println!("Playback speed set to {speed}x");
        } else {
            println!("Speed must be positive");
        }
    }

    pub fn load_recording(&self, name: &str) -> Option<Recording> {
        load_recording_from(&self.recordings_dir, name)
    }

    /// Replay a recorded session. Blocks until completion or hotkey stop.
    pub fn play_attack(&mut self, session_name: &str, speed: f64) -> bool {
        if self.is_playing() {
            tracing::warn!("Already playing an attack");
            return false;
        }
        let Some(recording) = self.load_recording(session_name) else {
            tracing::error!("Could not load recording: {session_name}");
            return false;
        };

        println!("\n=== PLAYING ATTACK SESSION: {session_name} ===");
        println!("Duration: {:.1}s, actions: {}, speed: {speed}x",
                 recording.duration, recording.actions.len());
        println!("Starting in 3 seconds...");
        println!("Press F8 to pause, F9 to stop, ESC for emergency stop");
        thread::sleep(Duration::from_secs(3));

        self.is_playing.store(true, Ordering::SeqCst);
        let flag = self.is_playing.clone();
        let handle = thread::spawn(move || {
            playback_loop(flag, recording, speed);
        });
        self.thread = Some(handle);
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
        true
    }

    pub fn stop_playback(&mut self) {
        self.is_playing.store(false, Ordering::SeqCst);
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
    }

    pub fn validate_recording(&self, session_name: &str) -> serde_json::Value {
        let Some(recording) = self.load_recording(session_name) else {
            return serde_json::json!({"valid": false, "error": "Recording not found"});
        };
        let count = recording.actions.len();
        if count == 0 {
            return serde_json::json!({"valid": false, "error": "No actions"});
        }
        let (sw, sh) = screen_size();
        let mut oob: Vec<(usize, i32, i32)> = Vec::new();
        for (i, a) in recording.actions.iter().enumerate() {
            if !(0..sw).contains(&a.x) || !(0..sh).contains(&a.y) {
                oob.push((i, a.x, a.y));
            }
        }
        serde_json::json!({
            "valid": oob.is_empty(),
            "total_actions": count,
            "duration": recording.duration,
            "out_of_bounds": oob,
        })
    }

    pub fn preview_recording(&self, session_name: &str) {
        let Some(recording) = self.load_recording(session_name) else {
            println!("Recording not found: {session_name}");
            return;
        };
        let actions = &recording.actions;
        println!("\n=== RECORDING PREVIEW: {session_name} ===");
        println!("Duration: {:.1}s, total actions: {}", recording.duration, actions.len());

        let mut counts: std::collections::BTreeMap<&str, usize> = Default::default();
        for a in actions {
            let key = action_kind_name(&a.kind);
            *counts.entry(key).or_insert(0) += 1;
        }
        println!("\nAction breakdown:");
        for (k, v) in counts {
            println!("  {k}: {v}");
        }
        println!("\nFirst 10 actions:");
        for (i, a) in actions.iter().take(10).enumerate() {
            let kind = action_kind_name(&a.kind);
            println!("  {:2}. {:6.1}s - {} at ({}, {})", i + 1, a.timestamp, kind, a.x, a.y);
        }
        if actions.len() > 10 {
            println!("  ... and {} more", actions.len() - 10);
        }
    }
}

/// Standalone playback used by both AttackPlayer and the auto-attacker.
pub fn play_recording(stop_flag: Arc<AtomicBool>, recording: Recording, speed: f64) {
    playback_loop(stop_flag, recording, speed);
}

fn action_kind_name(k: &ActionKind) -> &'static str {
    match k {
        ActionKind::Click => "click",
        ActionKind::Move => "move",
        ActionKind::Delay { .. } => "delay",
        ActionKind::Drag { .. } => "drag",
    }
}

fn playback_loop(is_playing: Arc<AtomicBool>, recording: Recording, speed: f64) {
    let mut enigo = match Enigo::new(&Settings::default()) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to init Enigo: {e}");
            is_playing.store(false, Ordering::SeqCst);
            return;
        }
    };
    let mut paused = false;
    let mut last_ts = 0.0_f64;

    for (i, action) in recording.actions.iter().enumerate() {
        if !is_playing.load(Ordering::SeqCst) {
            break;
        }
        if hotkeys::is_pressed(hotkeys::VK_ESCAPE) {
            println!("\nEmergency stop activated");
            break;
        }
        if hotkeys::is_pressed(hotkeys::VK_F9) {
            println!("\nPlayback stopped by user");
            break;
        }
        if hotkeys::is_pressed(hotkeys::VK_F8) {
            paused = !paused;
            println!("\nPlayback {}", if paused { "paused" } else { "resumed" });
            hotkeys::wait_for_release(hotkeys::VK_F8, 1000);
        }
        while paused && is_playing.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(100));
            if hotkeys::is_pressed(hotkeys::VK_F8) {
                paused = false;
                println!("Playback resumed");
                hotkeys::wait_for_release(hotkeys::VK_F8, 1000);
            }
        }
        if !is_playing.load(Ordering::SeqCst) {
            break;
        }

        if i > 0 {
            let delay = (action.timestamp - last_ts) / speed;
            if delay > 0.0 {
                thread::sleep(Duration::from_secs_f64(delay));
            }
        }
        execute_action(&mut enigo, action, speed);
        last_ts = action.timestamp;

        let progress = (i + 1) as f64 / recording.actions.len() as f64 * 100.0;
        print!("\rProgress: {:.1}% ({}/{})", progress, i + 1, recording.actions.len());
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }

    println!("\nPlayback completed");
    is_playing.store(false, Ordering::SeqCst);
}

fn execute_action(enigo: &mut Enigo, action: &Action, speed: f64) {
    match &action.kind {
        ActionKind::Click => {
            let _ = enigo.move_mouse(action.x, action.y, Coordinate::Abs);
            let _ = enigo.button(Button::Left, Direction::Click);
        }
        ActionKind::Move => {
            let _ = enigo.move_mouse(action.x, action.y, Coordinate::Abs);
        }
        ActionKind::Delay { duration } => {
            thread::sleep(Duration::from_secs_f64(duration / speed));
        }
        ActionKind::Drag { start_x, start_y } => {
            let _ = enigo.move_mouse(*start_x, *start_y, Coordinate::Abs);
            let _ = enigo.button(Button::Left, Direction::Press);
            let _ = enigo.move_mouse(action.x, action.y, Coordinate::Abs);
            let _ = enigo.button(Button::Left, Direction::Release);
        }
    }
}

#[cfg(windows)]
fn screen_size() -> (i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) }
}

#[cfg(not(windows))]
fn screen_size() -> (i32, i32) {
    (1920, 1080)
}

/// One-off click.
pub fn click_at(x: i32, y: i32) -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default())?;
    enigo.move_mouse(x, y, Coordinate::Abs)?;
    enigo.button(Button::Left, Direction::Click)?;
    Ok(())
}

