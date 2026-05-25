# COC Attack Bot (Rust)

A Rust rewrite of [DFanso/coc-attack-bot](https://github.com/DFanso/coc-attack-bot) — Windows automation for Clash of Clans with attack recording, AI-powered base analysis (Google Gemini), and an auto-attacker loop.

![Build](https://github.com/DFanso/coc-attack-bot-rs/actions/workflows/build.yml/badge.svg)

## Why Rust?

- **Single static binary (~10 MB)** instead of "install Python + pip install opencv + Pillow + pyautogui + pywin32"
- **Faster startup**, lower memory footprint
- **Native Win32 hotkey polling** (`GetAsyncKeyState`) with no GIL contention
- **`enigo`** for input simulation, **`xcap`** for window capture, **`reqwest`** for the Gemini API, **`image`** for resizing/encoding

## ⚠️ Disclaimer

This bot is for educational purposes. **Using third-party automation against Clash of Clans violates Supercell's Terms of Service and may result in your account being banned.** Use at your own risk. The author is not responsible for any consequences.

## Features

- 🎯 Coordinate mapping — record on-screen button positions via hotkeys
- 📹 Attack recording — capture mouse clicks with timing (auto or manual)
- ▶️ Attack playback — replay sessions with adjustable speed
- 🤖 AI base analysis via Google Gemini (vision)
- 🏃 Auto-attacker — find good bases, attack, return home, repeat
- 🖼️ Window-aware screen capture
- 🎮 Game window detection (Clash of Clans, Google Play Games for Windows, BlueStacks, Nox, LDPlayer, MEmu)
- ⌨️ F1/F2/F3/F5/F6/F7/F8/F9/Esc/Ctrl+Alt+S hotkeys

## Requirements

- Windows 10 or later
- **Clash of Clans running in fullscreen** (required for accurate coordinates)
  - Works inside Google Play Games for Windows, BlueStacks, Nox, LDPlayer, MEmu
- A Google Gemini API key (only if you want AI base analysis)

## Install

Download a binary from [Releases](https://github.com/DFanso/coc-attack-bot-rs/releases), or build from source:

```bash
git clone https://github.com/DFanso/coc-attack-bot-rs
cd coc-attack-bot-rs
cargo build --release
# Binary: target/release/coc-attack-bot.exe
```

## Quick start

1. Launch Clash of Clans in fullscreen (in GPG / BlueStacks / etc.)
2. Run `coc-attack-bot.exe` — a numbered menu appears
3. **Menu 5 (Game Detection)** — confirm the bot found your CoC window
4. **Menu 1 (Coordinate Mapping)** — map these buttons with F2:
   - `attack` — Main attack button on home screen
   - `find_a_match` — Search for opponents
   - `next_button` — Skip to next base
   - `return_home` — Go back to base after battle
   - `end_button` — End battle button
5. **Menu 2 (Attack Recording)** — F5 to start, perform attack, F5 to stop
6. **Menu 4 → Setup Auto Attack** — pick sessions, enable AI, set loot mins
7. **Menu 4 → Start Auto Attack**

Stop anytime with **Ctrl+Alt+S**.

## Hotkeys

| Key        | Action                                |
| ---------- | ------------------------------------- |
| F1         | Toggle coordinate-mapping mode        |
| F2         | Record current mouse position         |
| F3         | Save mapped coordinates               |
| F5         | Start/stop attack recording           |
| F6         | Manual click (during recording)       |
| F7         | Add a delay marker (during recording) |
| F8         | Pause/resume playback                 |
| F9         | Stop playback                         |
| ESC        | Cancel / emergency stop               |
| Ctrl+Alt+S | Auto-attacker emergency stop          |

## How auto-attack works

```
loop {
  1. click attack
  2. click find_a_match
  3. wait 5s
  4. screenshot game window → Gemini Vision → {gold, elixir, dark, TH level, ATTACK/SKIP}
  5a. SKIP  → click next_button → goto 2  (up to N times)
  5b. ATTACK → play recorded attack session
  6. wait 3 minutes
  7. click return_home
}
```

The AI step is optional — disable it in setup and every base is accepted.

## Config

`config.json` is created on first run. Key settings:

```jsonc
{
  "ai_analyzer": {
    "google_gemini_api_key": "AIza...",
    "enabled": true,
    "min_gold": 300000,
    "min_elixir": 300000,
    "min_dark_elixir": 2000
  },
  "auto_attacker": {
    "attack_sessions": ["barch_collector_raid"],
    "max_search_attempts": 10
  }
}
```

## Data layout

```
coc-attack-bot/
├─ coc-attack-bot.exe
├─ config.json                # auto-generated
├─ logs/                      # daily rotating logs
├─ coordinates/               # button_coordinates.json
├─ recordings/                # *.json attack sessions
└─ screenshots/               # captures
```

## Differences from the Python original

- Uses `xcap` for window capture (cross-platform; on Windows it sits on top of GDI).
- Hotkeys via `GetAsyncKeyState` polling (50 ms loop) — same approach as `keyboard.is_pressed()` in the original.
- Input simulation via `enigo` (wraps `SendInput` on Windows).
- Auto-attacker worker thread takes a snapshot of config/coords/recordings_dir on start and is fully self-contained — no shared `Arc<Mutex<>>` plumbing.
- Drops the OpenCV template-matching helper from the original (`find_template_on_screen`); the AI analyzer covers the same use case more flexibly.

## Building

Release profile is LTO + `opt-level = 3` + stripped, giving a ~10 MB binary on Windows x64.

```bash
cargo build --release
```

To cross-compile from Linux/macOS to Windows you'll need the MSVC target (no easy mingw path because `windows` crate requires MSVC headers).

## License

MIT
