use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::ai_analyzer::AiAnalyzer;
use crate::auto_attacker::{required_buttons, AutoAttacker};
use crate::config::Config;
use crate::coordinate_mapper::CoordinateMapper;
use crate::player::AttackPlayer;
use crate::recorder::AttackRecorder;
use crate::screen_capture::ScreenCapture;

pub struct App {
    pub config: Config,
    pub coords: CoordinateMapper,
    pub recorder: AttackRecorder,
    pub player: AttackPlayer,
    pub screen_capture: ScreenCapture,
    pub auto_attacker: AutoAttacker,
    pub running: bool,
}

impl App {
    pub fn new() -> anyhow::Result<Self> {
        let config = Config::load_or_create("config.json")?;
        Ok(Self {
            config,
            coords: CoordinateMapper::new()?,
            recorder: AttackRecorder::new(true)?,
            player: AttackPlayer::new("recordings"),
            screen_capture: ScreenCapture::new()?,
            auto_attacker: AutoAttacker::new(),
            running: true,
        })
    }

    pub fn run(&mut self) {
        self.banner();
        while self.running {
            self.main_menu();
            let choice = prompt("\nEnter your choice: ");
            match choice.trim() {
                "1" => self.coordinate_mapping_menu(),
                "2" => self.attack_recording_menu(),
                "3" => self.attack_playback_menu(),
                "4" => self.auto_attack_menu(),
                "5" => self.game_detection_menu(),
                "6" => self.screenshots_menu(),
                "7" => self.settings_menu(),
                "8" => self.help(),
                "9" => self.running = false,
                _ => println!("Invalid choice."),
            }
        }
        self.shutdown();
    }

    fn shutdown(&mut self) {
        tracing::info!("Shutting down");
        if self.recorder.is_recording() {
            self.recorder.stop_recording();
        }
        if self.player.is_playing() {
            self.player.stop_playback();
        }
        if self.auto_attacker.is_running() {
            self.auto_attacker.stop();
        }
    }

    fn banner(&self) {
        println!("{}", "=".repeat(60));
        println!("        COC ATTACK BOT (Rust) — Windows Automation");
        println!("{}", "=".repeat(60));
        println!("  Automated Clash of Clans attack recording and playback");
        println!("{}", "=".repeat(60));
    }

    fn main_menu(&self) {
        println!("\n{}", "=".repeat(40));
        println!("           MAIN MENU");
        println!("{}", "=".repeat(40));
        println!("1. Coordinate Mapping");
        println!("2. Attack Recording");
        println!("3. Attack Playback");
        println!("4. Auto Attack System");
        println!("5. Game Detection");
        println!("6. Screenshots");
        println!("7. Settings");
        println!("8. Help");
        println!("9. Exit");
        println!("{}", "=".repeat(40));
    }

    // ─── Coordinate mapping ────────────────────────────────────────────────
    fn coordinate_mapping_menu(&mut self) {
        loop {
            println!("\n{}", "=".repeat(40));
            println!("       COORDINATE MAPPING");
            println!("{}", "=".repeat(40));
            println!("1. Start coordinate mapping");
            println!("2. View mapped coordinates");
            println!("3. Export coordinates");
            println!("4. Import coordinates");
            println!("5. Back to main menu");
            println!("{}", "=".repeat(40));

            match prompt("Enter your choice: ").trim() {
                "1" => {
                    if let Err(e) = self.coords.start_mapping() {
                        tracing::error!("Mapping error: {e}");
                    }
                }
                "2" => self.coords.list(),
                "3" => {
                    let name = prompt("Enter export filename (without extension): ");
                    if !name.trim().is_empty() {
                        let path = PathBuf::from(format!("coordinates/{}.json", name.trim()));
                        let _ = self.coords.export(&path);
                    }
                }
                "4" => {
                    let path_str = prompt("Enter import filename: ");
                    let p = PathBuf::from(path_str.trim());
                    if p.exists() {
                        let merge = prompt("Merge with existing? (y/n): ");
                        let merge = merge.trim().eq_ignore_ascii_case("y");
                        if let Err(e) = self.coords.import(&p, merge) {
                            tracing::error!("Import failed: {e}");
                        }
                    } else {
                        println!("File not found.");
                    }
                }
                "5" => break,
                _ => println!("Invalid choice."),
            }
        }
    }

    // ─── Attack recording ──────────────────────────────────────────────────
    fn attack_recording_menu(&mut self) {
        loop {
            println!("\n{}", "=".repeat(40));
            println!("       ATTACK RECORDING");
            println!("{}", "=".repeat(40));
            let auto = if self.recorder.auto_detect_clicks { "ENABLED" } else { "DISABLED" };
            println!("Auto-detection: {auto}");
            println!("{}", "=".repeat(40));
            println!("1. Start new recording");
            println!("2. List recordings");
            println!("3. View recording info");
            println!("4. Delete recording");
            println!("5. Toggle auto-detection");
            println!("6. Back to main menu");
            println!("{}", "=".repeat(40));

            match prompt("Enter your choice: ").trim() {
                "1" => {
                    let name = prompt("Enter session name: ");
                    let name = name.trim().to_string();
                    if name.is_empty() {
                        println!("Session name required.");
                        continue;
                    }
                    self.recorder.start_recording(name);
                    println!("\nPress Enter when recording is complete...");
                    let _ = io::stdin().lock().read_line(&mut String::new());
                    self.recorder.stop_recording();
                }
                "2" => {
                    let sessions = self.recorder.list_sessions();
                    if sessions.is_empty() {
                        println!("No recorded sessions found.");
                    } else {
                        println!("\n=== RECORDED SESSIONS ===");
                        for (i, s) in sessions.iter().enumerate() {
                            println!("  {}. {s}", i + 1);
                        }
                    }
                }
                "3" => {
                    let name = prompt("Enter session name: ");
                    let name = name.trim();
                    if let Some(rec) = self.recorder.load_recording(name) {
                        println!("\n=== SESSION INFO: {name} ===");
                        println!("Created:  {}", rec.created);
                        println!("Duration: {:.1}s", rec.duration);
                        println!("Actions:  {}", rec.actions.len());
                    } else {
                        println!("Session not found.");
                    }
                }
                "4" => {
                    let name = prompt("Enter session name to delete: ");
                    let name = name.trim().to_string();
                    if name.is_empty() { continue; }
                    let confirm = prompt(&format!("Delete '{name}'? (y/n): "));
                    if confirm.trim().eq_ignore_ascii_case("y") {
                        if self.recorder.delete_recording(&name) {
                            println!("Deleted: {name}");
                        } else {
                            println!("Recording not found.");
                        }
                    }
                }
                "5" => {
                    self.recorder.auto_detect_clicks = !self.recorder.auto_detect_clicks;
                    let s = if self.recorder.auto_detect_clicks { "ENABLED" } else { "DISABLED" };
                    println!("Auto-click detection is now {s}");
                }
                "6" => break,
                _ => println!("Invalid choice."),
            }
        }
    }

    // ─── Attack playback ───────────────────────────────────────────────────
    fn attack_playback_menu(&mut self) {
        loop {
            println!("\n{}", "=".repeat(40));
            println!("       ATTACK PLAYBACK");
            println!("{}", "=".repeat(40));
            println!("1. Play attack");
            println!("2. Preview recording");
            println!("3. Validate recording");
            println!("4. Set playback speed");
            println!("5. Back to main menu");
            println!("{}", "=".repeat(40));

            match prompt("Enter your choice: ").trim() {
                "1" => {
                    let sessions = self.recorder.list_sessions();
                    if sessions.is_empty() {
                        println!("No recorded sessions available.");
                        continue;
                    }
                    println!("\nAvailable sessions:");
                    for (i, s) in sessions.iter().enumerate() {
                        println!("  {}. {s}", i + 1);
                    }
                    let idx_str = prompt("Select session number: ");
                    let Ok(idx) = idx_str.trim().parse::<usize>() else {
                        println!("Invalid input.");
                        continue;
                    };
                    if idx == 0 || idx > sessions.len() {
                        println!("Invalid session number.");
                        continue;
                    }
                    let name = &sessions[idx - 1];
                    let speed_str = prompt("Playback speed (1.0 = normal): ");
                    let speed: f64 = speed_str.trim().parse().unwrap_or(1.0);
                    println!("\nStarting playback of '{name}' at {speed}x speed");
                    println!("Make sure COC is visible and in the correct state!");
                    let _ = prompt("Press Enter to begin...");
                    self.player.play_attack(name, speed);
                }
                "2" => {
                    let name = prompt("Enter session name: ");
                    self.player.preview_recording(name.trim());
                }
                "3" => {
                    let name = prompt("Enter session name: ");
                    let v = self.player.validate_recording(name.trim());
                    println!("\n=== VALIDATION RESULT ===\n{}",
                             serde_json::to_string_pretty(&v).unwrap_or_default());
                }
                "4" => {
                    let s = prompt("Enter playback speed (0.1 - 5.0): ");
                    match s.trim().parse::<f64>() {
                        Ok(v) => self.player.set_playback_speed(v),
                        Err(_) => println!("Invalid speed value."),
                    }
                }
                "5" => break,
                _ => println!("Invalid choice."),
            }
        }
    }

    // ─── Auto attack ───────────────────────────────────────────────────────
    fn auto_attack_menu(&mut self) {
        loop {
            println!("\n{}", "=".repeat(40));
            println!("       AUTO ATTACK SYSTEM");
            println!("{}", "=".repeat(40));
            if self.auto_attacker.is_running() {
                println!("Status: RUNNING");
                let s = self.auto_attacker.snapshot_stats();
                let rate = if s.total_attacks > 0 {
                    100.0 * s.successful_attacks as f64 / s.total_attacks as f64
                } else { 0.0 };
                println!("Attacks: {} (Success: {:.1}%)", s.total_attacks, rate);
            } else {
                println!("Status: STOPPED");
            }
            println!("{}", "=".repeat(40));
            println!("1. Setup Auto Attack");
            println!("2. Start Auto Attack");
            println!("3. Stop Auto Attack");
            println!("4. View Statistics");
            println!("5. Configure Required Buttons");
            println!("6. Back to main menu");
            println!("{}", "=".repeat(40));

            match prompt("Enter your choice: ").trim() {
                "1" => self.setup_auto_attack(),
                "2" => self.start_auto_attack(),
                "3" => self.stop_auto_attack(),
                "4" => self.show_auto_stats(),
                "5" => self.configure_auto_buttons(),
                "6" => break,
                _ => println!("Invalid choice."),
            }
        }
    }

    fn setup_auto_attack(&mut self) {
        println!("\n=== AUTO ATTACK SETUP ===");
        let sessions = self.recorder.list_sessions();
        if sessions.is_empty() {
            println!("No recorded attack sessions found!");
            println!("Please record some attacks first using the Attack Recording menu.");
            return;
        }
        println!("Available attack sessions:");
        for (i, s) in sessions.iter().enumerate() {
            println!("  {}. {s}", i + 1);
        }
        let mut selected: Vec<String> = Vec::new();
        loop {
            let line = prompt("\nEnter session number to add (0 to finish): ");
            let trimmed = line.trim();
            if trimmed == "0" { break; }
            let Ok(n) = trimmed.parse::<usize>() else {
                println!("Please enter a valid number");
                continue;
            };
            if n == 0 || n > sessions.len() {
                println!("Invalid session number");
                continue;
            }
            let name = sessions[n - 1].clone();
            if !selected.contains(&name) {
                selected.push(name.clone());
                println!("Added: {name}");
            } else {
                println!("Already selected");
            }
        }
        if selected.is_empty() {
            println!("No sessions selected");
            return;
        }

        let use_ai_raw = prompt("\nEnable AI Analysis for this run? (y/n, default y): ");
        let use_ai = !use_ai_raw.trim().eq_ignore_ascii_case("n");
        self.config.ai_analyzer.enabled = use_ai;

        if use_ai {
            let mut api_key = self.config.ai_analyzer.google_gemini_api_key.clone();
            if api_key.is_empty() || api_key.starts_with("REPLACE_ME") {
                let entered = prompt("Please enter your Google Gemini API Key: ");
                let entered = entered.trim().to_string();
                if entered.is_empty() {
                    println!("❌ API Key cannot be empty. Disabling AI analysis.");
                    self.config.ai_analyzer.enabled = false;
                } else {
                    self.config.ai_analyzer.google_gemini_api_key = entered.clone();
                    api_key = entered;
                }
            }
            if self.config.ai_analyzer.enabled {
                println!("Testing AI Connection...");
                match AiAnalyzer::new(api_key) {
                    Ok(a) => {
                        if !a.test_connection() {
                            println!("❌ AI Connection Failed. Check your API key. Disabling AI for this run.");
                            self.config.ai_analyzer.enabled = false;
                        } else {
                            println!("✅ AI Connection Successful.");
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to init AI client: {e}");
                        self.config.ai_analyzer.enabled = false;
                    }
                }
            }
        }

        println!("\nSet minimum loot requirements:");
        let mg = prompt(&format!("Minimum Gold (default {}): ", self.config.ai_analyzer.min_gold));
        if let Ok(v) = mg.trim().parse::<u64>() { self.config.ai_analyzer.min_gold = v; }
        let me = prompt(&format!("Minimum Elixir (default {}): ", self.config.ai_analyzer.min_elixir));
        if let Ok(v) = me.trim().parse::<u64>() { self.config.ai_analyzer.min_elixir = v; }
        let md = prompt(&format!("Minimum Dark Elixir (default {}): ", self.config.ai_analyzer.min_dark_elixir));
        if let Ok(v) = md.trim().parse::<u64>() { self.config.ai_analyzer.min_dark_elixir = v; }

        self.config.auto_attacker.attack_sessions = selected.clone();
        if let Err(e) = self.config.save() {
            tracing::error!("Failed to save config: {e}");
        }

        println!("\n{}", "=".repeat(40));
        println!("✅ Auto Attack Configured:");
        println!("  Attack Sessions: {}", selected.join(", "));
        println!("  AI Analysis: {}", if self.config.ai_analyzer.enabled { "ENABLED" } else { "DISABLED" });
        if self.config.ai_analyzer.enabled {
            println!("    Min Gold:        {}", self.config.ai_analyzer.min_gold);
            println!("    Min Elixir:      {}", self.config.ai_analyzer.min_elixir);
            println!("    Min Dark Elixir: {}", self.config.ai_analyzer.min_dark_elixir);
        }
        println!("{}", "=".repeat(40));
        println!("Ready to start from the Auto Attack menu!");
    }

    fn start_auto_attack(&mut self) {
        if self.auto_attacker.is_running() {
            println!("Auto attack is already running!");
            return;
        }
        if self.config.auto_attacker.attack_sessions.is_empty() {
            println!("❌ No attack sessions configured! Please run Setup first.");
            return;
        }
        println!("\n{}", "=".repeat(40));
        println!("         🚀 STARTING AUTO ATTACK 🚀");
        println!("{}", "=".repeat(40));
        println!("Attack Sessions: {}", self.config.auto_attacker.attack_sessions.join(", "));
        println!("AI Analysis: {}",
                 if self.config.ai_analyzer.enabled { "ENABLED" } else { "DISABLED" });
        let confirm = prompt("Confirm and start auto attack? (y/n): ");
        if !confirm.trim().eq_ignore_ascii_case("y") {
            println!("Auto attack cancelled.");
            return;
        }
        let config = self.config.clone();
        let coords = self.coords.coordinates.clone();
        let recordings_dir = self.recorder.recordings_dir.clone();
        self.auto_attacker.start(config, coords, recordings_dir);
        println!("\n✅ Auto attack started successfully!");
        println!("Press Ctrl+Alt+S to stop at any time.");
    }

    fn stop_auto_attack(&mut self) {
        if !self.auto_attacker.is_running() {
            println!("Auto attack is not running");
            return;
        }
        println!("Stopping auto attack...");
        self.auto_attacker.stop();
        println!("Auto attack stopped");
    }

    fn show_auto_stats(&self) {
        let s = self.auto_attacker.snapshot_stats();
        let rate = if s.total_attacks > 0 {
            100.0 * s.successful_attacks as f64 / s.total_attacks as f64
        } else { 0.0 };
        let runtime_h = s.start_time.map(|t| t.elapsed().as_secs_f64() / 3600.0).unwrap_or(0.0);
        println!("\n{}", "=".repeat(50));
        println!("        AUTO ATTACK STATISTICS");
        println!("{}", "=".repeat(50));
        println!("Status:          {}", if self.auto_attacker.is_running() { "RUNNING" } else { "STOPPED" });
        println!("Total Attacks:   {}", s.total_attacks);
        println!("Successful:      {}", s.successful_attacks);
        println!("Failed:          {}", s.failed_attacks);
        println!("Success Rate:    {rate:.1}%");
        println!("Runtime:         {runtime_h:.1} hours");
        println!("Configured:      {}", self.config.auto_attacker.attack_sessions.join(", "));
        println!("{}", "=".repeat(50));
        let _ = prompt("\nPress Enter to continue...");
    }

    fn configure_auto_buttons(&self) {
        let required = required_buttons();
        println!("\n{}", "=".repeat(60));
        println!("        REQUIRED BUTTON MAPPINGS");
        println!("{}", "=".repeat(60));
        for (name, desc) in &required {
            let mapped = self.coords.coordinates.contains_key(*name);
            let status = if mapped { "✓ MAPPED" } else { "✗ MISSING" };
            println!("{name:20} | {status:10} | {desc}");
        }
        println!("{}", "=".repeat(60));
        println!("\nTo map missing buttons:");
        println!("1. Go to 'Coordinate Mapping' in the main menu");
        println!("2. Use F2 to record each button position");
        println!("3. Use the exact button names shown above");
        let _ = prompt("\nPress Enter to continue...");
    }

    // ─── Game detection ────────────────────────────────────────────────────
    fn game_detection_menu(&mut self) {
        println!("\n{}", "=".repeat(40));
        println!("       GAME DETECTION");
        println!("{}", "=".repeat(40));
        println!("Detecting COC game window...");
        match self.screen_capture.find_game_window() {
            Ok(Some(gw)) => {
                println!("Game window found!");
                println!("Title:    {}", gw.title);
                println!("Position: ({}, {})", gw.x, gw.y);
                println!("Size:     {} x {}", gw.width, gw.height);
                let shot = prompt("\nTake screenshot of game window? (y/n): ");
                if shot.trim().eq_ignore_ascii_case("y") {
                    match self.screen_capture.capture_game_screen() {
                        Ok(Some(p)) => println!("Screenshot saved: {}", p.display()),
                        _ => println!("Screenshot failed"),
                    }
                }
            }
            Ok(None) => println!("Game window not found.\nMake sure Clash of Clans is running and visible."),
            Err(e) => println!("Detection error: {e}"),
        }
        let _ = prompt("\nPress Enter to continue...");
    }

    // ─── Screenshots ───────────────────────────────────────────────────────
    fn screenshots_menu(&mut self) {
        loop {
            println!("\n{}", "=".repeat(40));
            println!("         SCREENSHOTS");
            println!("{}", "=".repeat(40));
            println!("1. Take full screen screenshot");
            println!("2. Take game window screenshot");
            println!("3. View screenshots folder");
            println!("4. Back to main menu");
            println!("{}", "=".repeat(40));

            match prompt("Enter your choice: ").trim() {
                "1" => {
                    match self.screen_capture.capture_full_screen() {
                        Ok(p) => println!("Screenshot saved: {}", p.display()),
                        Err(e) => println!("Failed: {e}"),
                    }
                }
                "2" => {
                    match self.screen_capture.capture_game_screen() {
                        Ok(Some(p)) => println!("Screenshot saved: {}", p.display()),
                        Ok(None) => println!("Game window not found."),
                        Err(e) => println!("Failed: {e}"),
                    }
                }
                "3" => {
                    let dir = PathBuf::from("screenshots");
                    if let Ok(entries) = std::fs::read_dir(&dir) {
                        let mut files: Vec<String> = entries
                            .flatten()
                            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("png"))
                            .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                            .collect();
                        files.sort();
                        let total = files.len();
                        if total == 0 {
                            println!("No screenshots found.");
                        } else {
                            println!("\nScreenshots in {}:", dir.display());
                            for f in files.iter().rev().take(10) {
                                println!("  {f}");
                            }
                            if total > 10 {
                                println!("  ... and {} more", total - 10);
                            }
                        }
                    } else {
                        println!("Screenshots directory not found.");
                    }
                }
                "4" => break,
                _ => println!("Invalid choice."),
            }
        }
    }

    fn settings_menu(&self) {
        println!("\n{}", "=".repeat(40));
        println!("          SETTINGS");
        println!("{}", "=".repeat(40));
        println!("Bot:            {} v{}", self.config.bot.name, self.config.bot.version);
        println!("Click delay:    {}s", self.config.automation.default_click_delay);
        println!("Playback speed: {}x", self.config.automation.default_playback_speed);
        println!("Screenshot fmt: {}", self.config.automation.screenshot_format);
        println!("Failsafe:       {}", self.config.automation.failsafe_enabled);
        println!("Logs dir:       {}", self.config.directories.logs);
        println!("AI enabled:     {}", self.config.ai_analyzer.enabled);
        println!("\nEdit config.json directly to change values, then restart.");
        let _ = prompt("Press Enter to continue...");
    }

    fn help(&self) {
        println!("\n{}", "=".repeat(60));
        println!("                    HELP");
        println!("{}", "=".repeat(60));
        println!(r#"
GETTING STARTED:
1. Open Clash of Clans in full screen
2. Use 'Game Detection' to verify the bot can find your game
3. Map button coordinates for your screen resolution
4. Record attack sessions
5. Set up and start auto attack

COORDINATE MAPPING:
  F1   start/stop mapping mode
  F2   record current mouse position
  F3   save coordinates
  ESC  cancel

ATTACK RECORDING:
  F5   start/stop
  F6   manual click
  F7   add delay
  ESC  cancel

ATTACK PLAYBACK:
  F8   pause/resume
  F9   stop
  ESC  emergency stop

AUTO ATTACK SYSTEM:
  Ctrl+Alt+S — emergency stop

REQUIRED BUTTONS FOR AUTO ATTACK:
  attack, find_a_match, next_button, return_home, end_button
"#);
        let _ = prompt("\nPress Enter to continue...");
        thread::sleep(Duration::from_millis(50));
    }
}

fn prompt(label: &str) -> String {
    print!("{label}");
    io::stdout().flush().ok();
    let mut s = String::new();
    io::stdin().lock().read_line(&mut s).ok();
    s
}
