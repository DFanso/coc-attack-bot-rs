use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::ai_analyzer::AiAnalyzer;
use crate::config::Config;
use crate::coordinate_mapper::Coord;
use crate::hotkeys;
use crate::player::{click_at, play_recording};
use crate::recorder::load_recording_from;
use crate::screen_capture::ScreenCapture;

#[derive(Debug, Default, Clone)]
pub struct Stats {
    pub total_attacks: u32,
    pub successful_attacks: u32,
    pub failed_attacks: u32,
    pub start_time: Option<Instant>,
    pub last_attack_time: Option<Instant>,
}

pub struct AutoAttacker {
    is_running: Arc<AtomicBool>,
    pub stats: Arc<Mutex<Stats>>,
    thread: Option<JoinHandle<()>>,
}

/// Snapshot of everything the worker needs to run independently of the UI.
struct WorkerInput {
    config: Config,
    coords: BTreeMap<String, Coord>,
    recordings_dir: PathBuf,
}

impl AutoAttacker {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(Mutex::new(Stats::default())),
            thread: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn start(
        &mut self,
        config: Config,
        coords: BTreeMap<String, Coord>,
        recordings_dir: PathBuf,
    ) {
        if self.is_running() {
            println!("Auto attacker already running");
            return;
        }
        if config.auto_attacker.attack_sessions.is_empty() {
            tracing::error!("No attack sessions configured");
            return;
        }
        self.is_running.store(true, Ordering::SeqCst);
        {
            let mut s = self.stats.lock().unwrap();
            *s = Stats {
                start_time: Some(Instant::now()),
                ..Default::default()
            };
        }

        let flag = self.is_running.clone();
        let stats = self.stats.clone();
        let input = WorkerInput { config, coords, recordings_dir };

        let handle = thread::spawn(move || {
            auto_attack_loop(flag, stats, input);
        });
        self.thread = Some(handle);
        tracing::info!("Auto attacker started");
    }

    pub fn stop(&mut self) {
        if !self.is_running() {
            return;
        }
        tracing::info!("Auto attacker stopping...");
        self.is_running.store(false, Ordering::SeqCst);
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
        tracing::info!("Auto attacker stopped");
    }

    pub fn snapshot_stats(&self) -> Stats {
        self.stats.lock().unwrap().clone()
    }
}

pub fn required_buttons() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("attack", "Main attack button on home screen");
    m.insert("find_a_match", "Find match/search button in attack screen");
    m.insert("next_button", "Next button to skip bases with low loot");
    m.insert("return_home", "Return home button after battle completion");
    m.insert("end_button", "End battle button (restart search loop)");
    m
}

fn auto_attack_loop(
    is_running: Arc<AtomicBool>,
    stats: Arc<Mutex<Stats>>,
    input: WorkerInput,
) {
    // Worker owns its own ScreenCapture and AiAnalyzer.
    let mut screen_capture = match ScreenCapture::new() {
        Ok(sc) => sc,
        Err(e) => {
            tracing::error!("Failed to init ScreenCapture: {e}");
            is_running.store(false, Ordering::SeqCst);
            return;
        }
    };
    let ai_analyzer = match AiAnalyzer::new(input.config.ai_analyzer.google_gemini_api_key.clone()) {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("Failed to init AiAnalyzer: {e}");
            is_running.store(false, Ordering::SeqCst);
            return;
        }
    };

    let mut session_idx: usize = 0;

    while is_running.load(Ordering::SeqCst) {
        if hotkeys::emergency_stop_pressed() {
            tracing::warn!("Emergency stop activated!");
            break;
        }
        tracing::info!("🎯 Starting new attack cycle...");

        let result = execute_attack_sequence(
            &is_running,
            &input,
            &mut session_idx,
            &mut screen_capture,
            &ai_analyzer,
        );

        {
            let mut s = stats.lock().unwrap();
            s.total_attacks += 1;
            if result {
                s.successful_attacks += 1;
                tracing::info!("✅ Attack sequence completed successfully");
            } else {
                s.failed_attacks += 1;
                tracing::warn!("❌ Attack sequence failed");
            }
            s.last_attack_time = Some(Instant::now());
        }

        if is_running.load(Ordering::SeqCst) {
            let delay = 5 + (Instant::now().elapsed().as_nanos() as u64 % 11);
            tracing::info!("⏳ Waiting {delay} seconds before next attack...");
            for _ in 0..delay {
                if !is_running.load(Ordering::SeqCst) { break; }
                thread::sleep(Duration::from_secs(1));
            }
        }
    }
    is_running.store(false, Ordering::SeqCst);
}

fn execute_attack_sequence(
    is_running: &Arc<AtomicBool>,
    input: &WorkerInput,
    session_idx: &mut usize,
    screen_capture: &mut ScreenCapture,
    ai_analyzer: &AiAnalyzer,
) -> bool {
    let coords = &input.coords;

    // Step 1: Click attack button
    let Some(attack_coord) = coords.get("attack") else {
        tracing::error!("Attack button not mapped");
        return false;
    };
    tracing::info!("1️⃣ Clicking attack button at ({}, {})", attack_coord.x, attack_coord.y);
    let _ = click_at(attack_coord.x, attack_coord.y);
    thread::sleep(Duration::from_secs(2));

    // Steps 2-6: Find good loot target
    if !find_good_loot_target(is_running, &input.config, coords, screen_capture, ai_analyzer) {
        tracing::warn!("Could not find good loot target");
        return false;
    }

    // Step 7: Play next recorded attack
    let session_name = next_session(session_idx, &input.config.auto_attacker.attack_sessions);
    if session_name.is_empty() {
        return false;
    }
    let Some(recording) = load_recording_from(&input.recordings_dir, &session_name) else {
        tracing::error!("Could not load recording: {session_name}");
        return false;
    };
    tracing::info!("🎯 Playing attack session: {session_name}");

    // Use a per-playback stop flag tied to is_running.
    let playback_flag = Arc::new(AtomicBool::new(true));
    let pb_handle = {
        let f = playback_flag.clone();
        let rec = recording.clone();
        thread::spawn(move || play_recording(f, rec, 1.0))
    };
    // While playback runs, watch for outer stop / emergency.
    while pb_handle.is_finished() == false {
        if !is_running.load(Ordering::SeqCst) || hotkeys::emergency_stop_pressed() {
            playback_flag.store(false, Ordering::SeqCst);
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }
    let _ = pb_handle.join();

    if !is_running.load(Ordering::SeqCst) {
        return false;
    }

    // Step 8: Wait 3 minutes for battle completion
    tracing::info!("⏳ Waiting 3 minutes for battle completion...");
    let mut remaining: i64 = 180;
    while remaining > 0 {
        if !is_running.load(Ordering::SeqCst) { break; }
        tracing::info!("⏳ Battle in progress... {}m {}s remaining",
                       remaining / 60, remaining % 60);
        let step = remaining.min(10) as u64;
        thread::sleep(Duration::from_secs(step));
        remaining -= step as i64;
    }

    // Step 9: Return home
    return_home(coords);
    true
}

fn find_good_loot_target(
    is_running: &Arc<AtomicBool>,
    config: &Config,
    coords: &BTreeMap<String, Coord>,
    screen_capture: &mut ScreenCapture,
    ai_analyzer: &AiAnalyzer,
) -> bool {
    let max_attempts = config.auto_attacker.max_search_attempts.max(1);

    let Some(find_coord) = coords.get("find_a_match").copied() else {
        tracing::error!("find_a_match button not mapped");
        return false;
    };
    if !coords.contains_key("next_button") {
        tracing::error!("next_button not mapped");
        return false;
    }

    for attempt in 1..=max_attempts {
        if !is_running.load(Ordering::SeqCst) { return false; }

        tracing::info!("2️⃣ Clicking find_a_match at ({}, {}) — Attempt {attempt}/{max_attempts}",
                       find_coord.x, find_coord.y);
        let _ = click_at(find_coord.x, find_coord.y);

        tracing::info!("3️⃣ Waiting 5 seconds for base to load...");
        thread::sleep(Duration::from_secs(5));

        let Ok(Some(screenshot_path)) = screen_capture.capture_game_screen() else {
            tracing::warn!("Could not take screenshot, skipping base...");
            continue;
        };

        let use_ai = config.ai_analyzer.enabled;
        tracing::info!("AI Analysis is {}.", if use_ai { "ENABLED" } else { "DISABLED" });

        let decision = if use_ai {
            tracing::info!("4️⃣ Checking enemy loot with AI...");
            check_loot_with_ai(&screenshot_path, config, ai_analyzer)
        } else {
            tracing::info!("4️⃣ AI disabled — accepting base.");
            true
        };

        if decision {
            tracing::info!("✅ Base is good! Proceeding with attack!");
            return true;
        }
        tracing::info!("❌ Base not suitable. Clicking next...");
        if let Some(next_coord) = coords.get("next_button") {
            let _ = click_at(next_coord.x, next_coord.y);
            thread::sleep(Duration::from_secs(3));
        }
    }

    tracing::warn!("Could not find good loot after {max_attempts} attempts");
    tracing::info!("🔄 Clicking end button to restart search...");
    if let Some(end_coord) = coords.get("end_button") {
        let _ = click_at(end_coord.x, end_coord.y);
        thread::sleep(Duration::from_secs(3));
    } else {
        tracing::warn!("end_button not mapped — cannot retry automatically");
        return false;
    }

    tracing::info!("🔄 Retrying base search after end button...");
    search_for_good_base_cycle(is_running, config, coords, screen_capture, ai_analyzer)
}

fn search_for_good_base_cycle(
    is_running: &Arc<AtomicBool>,
    config: &Config,
    coords: &BTreeMap<String, Coord>,
    screen_capture: &mut ScreenCapture,
    ai_analyzer: &AiAnalyzer,
) -> bool {
    let max_attempts = config.auto_attacker.max_search_attempts.max(1);
    let Some(find_coord) = coords.get("find_a_match").copied() else { return false; };
    let Some(next_coord) = coords.get("next_button").copied() else { return false; };

    for attempt in 1..=max_attempts {
        if !is_running.load(Ordering::SeqCst) { return false; }
        tracing::info!("2️⃣ Clicking find_a_match — Attempt {attempt}/{max_attempts}");
        let _ = click_at(find_coord.x, find_coord.y);
        thread::sleep(Duration::from_secs(5));

        let Ok(Some(screenshot_path)) = screen_capture.capture_game_screen() else {
            tracing::warn!("Could not take screenshot, skipping base...");
            continue;
        };
        let use_ai = config.ai_analyzer.enabled;
        let decision = if use_ai {
            check_loot_with_ai(&screenshot_path, config, ai_analyzer)
        } else {
            true
        };
        if decision {
            return true;
        }
        let _ = click_at(next_coord.x, next_coord.y);
        thread::sleep(Duration::from_secs(3));
    }
    false
}

fn check_loot_with_ai(
    screenshot_path: &std::path::Path,
    config: &Config,
    ai_analyzer: &AiAnalyzer,
) -> bool {
    let min_gold = config.ai_analyzer.min_gold;
    let min_elixir = config.ai_analyzer.min_elixir;
    let min_dark = config.ai_analyzer.min_dark_elixir;

    let analysis = ai_analyzer.analyze_base(screenshot_path, min_gold, min_elixir, min_dark);
    if analysis.error {
        tracing::error!("AI analysis failed: {}", analysis.reasoning);
        return false;
    }

    let gold = analysis.loot.gold;
    let elixir = analysis.loot.elixir;
    let dark = analysis.loot.dark_elixir;
    let th = analysis.townhall_level;

    tracing::info!("🔍 AI Extracted Loot: Gold={gold}, Elixir={elixir}, Dark={dark}");
    tracing::info!("🏰 Town Hall Level: {th}");
    tracing::info!("📋 Requirements: Gold={min_gold}, Elixir={min_elixir}, Dark={min_dark}, Max TH=12");

    if th > 12 {
        tracing::info!("❌ Overriding AI: Town Hall {th} too strong (max 12)");
        return false;
    }

    analysis.recommendation.eq_ignore_ascii_case("ATTACK")
}

fn return_home(coords: &BTreeMap<String, Coord>) {
    tracing::info!("🏠 Returning to home base...");
    if let Some(home) = coords.get("return_home") {
        tracing::info!("Clicking return_home at ({}, {})", home.x, home.y);
        let _ = click_at(home.x, home.y);
        thread::sleep(Duration::from_secs(5));
    } else {
        tracing::warn!("return_home button not mapped");
    }
}

fn next_session(idx: &mut usize, sessions: &[String]) -> String {
    if sessions.is_empty() {
        return String::new();
    }
    let name = sessions[*idx % sessions.len()].clone();
    *idx = (*idx + 1) % sessions.len();
    name
}
