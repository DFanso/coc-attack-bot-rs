use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

use crate::hotkeys;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Coord {
    pub x: i32,
    pub y: i32,
}

pub struct CoordinateMapper {
    coords_file: PathBuf,
    pub coordinates: BTreeMap<String, Coord>,
}

impl CoordinateMapper {
    pub fn new() -> Result<Self> {
        let dir = PathBuf::from("coordinates");
        std::fs::create_dir_all(&dir)?;
        let coords_file = dir.join("button_coordinates.json");

        let coordinates: BTreeMap<String, Coord> = if coords_file.exists() {
            match std::fs::read(&coords_file).and_then(|b| {
                serde_json::from_slice(&b).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            }) {
                Ok(m) => {
                    tracing::info!("Loaded {} coordinate mappings", BTreeMap::<String, Coord>::len(&m));
                    m
                }
                Err(e) => {
                    tracing::warn!("Failed to load coordinates: {e}");
                    BTreeMap::new()
                }
            }
        } else {
            tracing::info!("No existing coordinates file found");
            BTreeMap::new()
        };

        Ok(Self {
            coords_file,
            coordinates,
        })
    }

    pub fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.coordinates)?;
        std::fs::write(&self.coords_file, json)?;
        tracing::info!(
            "Coordinates saved to {} ({} total)",
            self.coords_file.display(),
            self.coordinates.len()
        );
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get(&self, name: &str) -> Option<Coord> {
        self.coordinates.get(name).copied()
    }

    pub fn list(&self) {
        if self.coordinates.is_empty() {
            println!("No coordinates mapped yet");
            return;
        }
        println!("\n=== MAPPED COORDINATES ===");
        for (name, c) in &self.coordinates {
            println!("  {name}: ({}, {})", c.x, c.y);
        }
        println!("Total: {} mappings", self.coordinates.len());
    }

    pub fn export(&self, path: &PathBuf) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.coordinates)?;
        std::fs::write(path, json)?;
        println!("Coordinates exported to {}", path.display());
        Ok(())
    }

    pub fn import(&mut self, path: &PathBuf, merge: bool) -> Result<()> {
        let bytes = std::fs::read(path).context("read import file")?;
        let imported: BTreeMap<String, Coord> = serde_json::from_slice(&bytes)?;
        if merge {
            for (k, v) in imported {
                self.coordinates.insert(k, v);
            }
        } else {
            self.coordinates = imported;
        }
        self.save()?;
        Ok(())
    }

    /// Interactive hotkey-driven mapping. Mirrors the Python F1/F2/F3/Esc flow.
    pub fn start_mapping(&mut self) -> Result<()> {
        let mut session: BTreeMap<String, Coord> = BTreeMap::new();

        println!("\n=== COORDINATE MAPPING MODE ===");
        println!("Instructions:");
        println!("  1. Move mouse to the button you want to map");
        println!("  2. Press F2 to record the position");
        println!("  3. Enter a name for the button (in this terminal)");
        println!("  4. Repeat for all buttons");
        println!("  5. Press F3 to save all mappings");
        println!("  6. Press ESC or F1 to exit");
        println!("\nStarting in 3 seconds...");
        std::thread::sleep(Duration::from_secs(3));

        let stdin = io::stdin();
        let mut stdout = io::stdout();

        loop {
            if hotkeys::is_pressed(hotkeys::VK_ESCAPE) {
                println!("\nMapping cancelled");
                break;
            }
            if hotkeys::is_pressed(hotkeys::VK_F1) {
                println!("\nExiting mapping mode");
                hotkeys::wait_for_release(hotkeys::VK_F1, 1000);
                break;
            }
            if hotkeys::is_pressed(hotkeys::VK_F2) {
                let (x, y) = cursor_position();
                print!("\nMouse at ({x}, {y}). Enter button name: ");
                stdout.flush().ok();
                let mut name = String::new();
                stdin.lock().read_line(&mut name).ok();
                let name = name.trim().to_string();
                if !name.is_empty() {
                    session.insert(name.clone(), Coord { x, y });
                    println!("Recorded '{name}' at ({x}, {y}). Session mappings: {}", session.len());
                }
                hotkeys::wait_for_release(hotkeys::VK_F2, 1000);
            }
            if hotkeys::is_pressed(hotkeys::VK_F3) {
                if session.is_empty() {
                    println!("\nNo mappings to save");
                } else {
                    let n = session.len();
                    for (k, v) in session.drain_filter_collect() {
                        self.coordinates.insert(k, v);
                    }
                    self.save()?;
                    println!("\nSaved {n} new mappings");
                }
                hotkeys::wait_for_release(hotkeys::VK_F3, 1000);
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        // Offer to save any remaining mappings
        if !session.is_empty() {
            print!("Save {} unsaved mappings? (y/n): ", session.len());
            stdout.flush().ok();
            let mut answer = String::new();
            stdin.lock().read_line(&mut answer).ok();
            if answer.trim().eq_ignore_ascii_case("y") {
                for (k, v) in session.drain_filter_collect() {
                    self.coordinates.insert(k, v);
                }
                self.save()?;
            }
        }
        Ok(())
    }
}

/// Get current cursor position via Win32.
#[cfg(windows)]
fn cursor_position() -> (i32, i32) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT { x: 0, y: 0 };
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

#[cfg(not(windows))]
fn cursor_position() -> (i32, i32) {
    (0, 0)
}

// Helper because BTreeMap doesn't have drain() until very recent stdlib;
// a tiny shim that yields owned (k, v) pairs and empties the map.
trait DrainFilterCollect<K: Ord + Clone, V: Clone> {
    fn drain_filter_collect(&mut self) -> Vec<(K, V)>;
}

impl<K: Ord + Clone, V: Clone> DrainFilterCollect<K, V> for BTreeMap<K, V> {
    fn drain_filter_collect(&mut self) -> Vec<(K, V)> {
        let out: Vec<(K, V)> = self.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        self.clear();
        out
    }
}
