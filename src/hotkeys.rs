//! Win32-based hotkey polling helpers. Mirrors the Python `keyboard.is_pressed()` polling
//! style used throughout the original bot.

#![cfg(windows)]

use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VIRTUAL_KEY};

pub const VK_ESCAPE: u16 = 0x1B;
pub const VK_F1: u16 = 0x70;
pub const VK_F2: u16 = 0x71;
pub const VK_F3: u16 = 0x72;
pub const VK_F5: u16 = 0x74;
pub const VK_F6: u16 = 0x75;
pub const VK_F7: u16 = 0x76;
pub const VK_F8: u16 = 0x77;
pub const VK_F9: u16 = 0x78;
pub const VK_LBUTTON: u16 = 0x01;
pub const VK_RBUTTON: u16 = 0x02;
pub const VK_CONTROL: u16 = 0x11;
pub const VK_MENU: u16 = 0x12; // Alt
pub const VK_S: u16 = 0x53;

/// True if the most-significant bit of GetAsyncKeyState is set (key currently down).
pub fn is_pressed(vk: u16) -> bool {
    unsafe { (GetAsyncKeyState(VIRTUAL_KEY(vk).0 as i32) as u16 & 0x8000) != 0 }
}

/// Block until the given vk is released. Caps total wait at `max_ms` so a stuck key can't lock us.
pub fn wait_for_release(vk: u16, max_ms: u64) {
    let mut waited = 0u64;
    while is_pressed(vk) && waited < max_ms {
        std::thread::sleep(std::time::Duration::from_millis(20));
        waited += 20;
    }
}

/// Ctrl+Alt+S emergency-stop chord.
pub fn emergency_stop_pressed() -> bool {
    is_pressed(VK_CONTROL) && is_pressed(VK_MENU) && is_pressed(VK_S)
}
