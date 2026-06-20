//! Global hotkey system using Windows low-level keyboard hook.
//!
//! This module implements:
//! - Low-level keyboard hook (WH_KEYBOARD_LL) to capture global key events
//! - Hotkey action dispatcher with debounce and access policy
//! - Key combination parsing (e.g., "alt+1", "home")
//! - Wait-for-key-release mechanism to prevent double-trigger

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use parking_lot::Mutex;
use anyhow::{Result, anyhow};

#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx,
    HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    GetMessageW, MSG,
};
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::*;

use crate::constants::{ACTION_DEBOUNCE, ALLOWED_HOTKEYS_WHEN_READER_OFF};

/// Hotkey event sent from hook thread to main logic
#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    KeyDown { vk_code: u32, modifiers: Modifiers },
    KeyUp { vk_code: u32 },
}

/// Modifier keys state
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

/// Parsed hotkey binding (modifier + key)
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedHotkey {
    modifiers: Modifiers,
    vk_code: u32,
}

/// Hotkey manager handle
pub struct HotkeyManager {
    inner: Arc<Mutex<HotkeyManagerInner>>,
    #[cfg(windows)]
    hook_handle: Arc<Mutex<Option<isize>>>,
}

struct HotkeyManagerInner {
    bindings: HashMap<String, ParsedHotkey>,
    last_action_time: Option<Instant>,
    wait_for_key_release: bool,
    reader_enabled: bool,
    action_callback: Option<Box<dyn Fn(&str) + Send + Sync>>,
}

/// Lock-free dedup: tracks the last dispatched action timestamp (ms).
/// Avoids double-execution when both the global hook AND the in-app handler
/// fire for the same key press in the main window.
static LAST_DISPATCH_TS: AtomicU64 = AtomicU64::new(0);

impl HotkeyManager {
    pub fn new(key_bindings: HashMap<String, String>) -> Result<Self> {
        let mut bindings = HashMap::new();
        for (action, key_str) in key_bindings {
            match parse_hotkey(&key_str) {
                Ok(parsed) => { bindings.insert(action, parsed); }
                Err(e) => { tracing::warn!("Failed to parse hotkey '{}' for action '{}': {}", key_str, action, e); }
            }
        }
        let inner = Arc::new(Mutex::new(HotkeyManagerInner {
            bindings,
            last_action_time: None,
            wait_for_key_release: false,
            reader_enabled: false,
            action_callback: None,
        }));
        Ok(Self {
            inner,
            #[cfg(windows)]
            hook_handle: Arc::new(Mutex::new(None)),
        })
    }

    pub fn set_action_callback<F>(&self, callback: F)
    where F: Fn(&str) + Send + Sync + 'static {
        self.inner.lock().action_callback = Some(Box::new(callback));
    }

    pub fn set_reader_enabled(&self, enabled: bool) {
        self.inner.lock().reader_enabled = enabled;
    }

    pub fn update_bindings(&self, key_bindings: HashMap<String, String>) -> Result<()> {
        let mut bindings = HashMap::new();
        for (action, key_str) in key_bindings {
            match parse_hotkey(&key_str) {
                Ok(parsed) => { bindings.insert(action, parsed); }
                Err(e) => { tracing::warn!("Failed to parse hotkey '{}' for action '{}': {}", key_str, action, e); }
            }
        }
        self.inner.lock().bindings = bindings;
        Ok(())
    }

    #[cfg(windows)]
    pub fn start(&self) -> Result<()> {
        use std::sync::atomic::{AtomicBool, Ordering};
        let inner = Arc::clone(&self.inner);
        let hook_handle = Arc::clone(&self.hook_handle);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        std::thread::spawn(move || {
            unsafe {
                HOOK_MANAGER_INNER = Some(Arc::clone(&inner));
                let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0);
                match hook {
                    Ok(hook) => {
                        *hook_handle.lock() = Some(hook.0 as isize);
                        let mut msg = MSG::default();
                        while running_clone.load(Ordering::Relaxed) {
                            let ret = GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0);
                            if ret.0 == -1 || ret.0 == 0 { break; }
                        }
                        let _ = UnhookWindowsHookEx(hook);
                    }
                    Err(e) => { tracing::error!("Failed to install keyboard hook: {:?}", e); }
                }
                HOOK_MANAGER_INNER = None;
            }
        });
        Ok(())
    }

    #[cfg(windows)]
    pub fn stop(&self) {
        if let Some(hook_ptr) = self.hook_handle.lock().take() {
            unsafe { let _ = UnhookWindowsHookEx(HHOOK(hook_ptr as *mut _)); }
        }
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) { #[cfg(windows)] self.stop(); }
}

fn is_action_allowed(action: &str, reader_enabled: bool) -> bool {
    if reader_enabled { true } else { ALLOWED_HOTKEYS_WHEN_READER_OFF.contains(&action) }
}

fn parse_hotkey(key_str: &str) -> Result<ParsedHotkey> {
    let lowercase = key_str.to_lowercase();
    let parts: Vec<&str> = lowercase.split('+').collect();
    if parts.is_empty() { return Err(anyhow!("Empty hotkey string")); }
    let mut modifiers = Modifiers::default();
    let key_part = parts.last().unwrap();
    for part in &parts[..parts.len() - 1] {
        match *part {
            "ctrl" => modifiers.ctrl = true,
            "alt" => modifiers.alt = true,
            "shift" => modifiers.shift = true,
            _ => return Err(anyhow!("Unknown modifier: {}", part)),
        }
    }
    let vk_code = key_name_to_vk(key_part)?;
    Ok(ParsedHotkey { modifiers, vk_code })
}

fn key_name_to_vk(key: &str) -> Result<u32> {
    if key.len() == 1 {
        let c = key.chars().next().unwrap();
        if c.is_ascii_lowercase() { return Ok(0x41 + (c as u32 - 'a' as u32)); }
        if c.is_ascii_digit() { return Ok(0x30 + (c as u32 - '0' as u32)); }
    }
    let vk = match key {
        "f1" => 0x70, "f2" => 0x71, "f3" => 0x72, "f4" => 0x73,
        "f5" => 0x74, "f6" => 0x75, "f7" => 0x76, "f8" => 0x77,
        "f9" => 0x78, "f10" => 0x79, "f11" => 0x7A, "f12" => 0x7B,
        "home" => 0x24, "end" => 0x23, "insert" => 0x2D, "delete" => 0x2E,
        "page_up" => 0x21, "page_down" => 0x22, "tab" => 0x09,
        "backspace" => 0x08, "space" => 0x20,
        "`" => 0xC0, "_" => 0xBD, "'" => 0xDE,
        _ => return Err(anyhow!("Unknown key name: {}", key)),
    };
    Ok(vk)
}

#[cfg(windows)]
static mut HOOK_MANAGER_INNER: Option<Arc<Mutex<HotkeyManagerInner>>> = None;

// The hook fires for ALL key presses in ALL windows (no foreground check).
// In the main window the in-app handler also fires, but LAST_DISPATCH_TS
// prevents double execution.
#[cfg(windows)]
unsafe extern "system" fn keyboard_hook_proc(
    n_code: i32, w_param: WPARAM, l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let kb = *(l_param.0 as *const KBDLLHOOKSTRUCT);
        let vk = kb.vkCode;
        let ctrl = (GetAsyncKeyState(0x11) as u16 & 0x8000) != 0;
        let alt = (GetAsyncKeyState(0x12) as u16 & 0x8000) != 0;
        let shift = (GetAsyncKeyState(0x10) as u16 & 0x8000) != 0;

        let is_down = matches!(w_param.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
        let is_up = matches!(w_param.0 as u32, WM_KEYUP | WM_SYSKEYUP);

        if let Some(inner) = (*std::ptr::addr_of!(HOOK_MANAGER_INNER)).as_ref() {
            if is_down {
                let mut m = inner.lock();
                if m.wait_for_key_release {
                    // Auto-release after 2s in case a KeyUp was missed.
                    if let Some(t) = m.last_action_time {
                        if Instant::now().duration_since(t) >= Duration::from_secs(2) {
                            m.wait_for_key_release = false;
                        }
                    }
                }
                if !m.wait_for_key_release {
                    let action = m.bindings.iter()
                        .find(|(_, b)| b.vk_code == vk && b.modifiers.ctrl == ctrl && b.modifiers.alt == alt && b.modifiers.shift == shift)
                        .map(|(a, _)| a.clone());
                    if let Some(action) = action {
                        if is_action_allowed(&action, m.reader_enabled) {
                            let now = Instant::now();
                            let ok = m.last_action_time.map_or(true, |t| now.duration_since(t) >= Duration::from_secs_f32(ACTION_DEBOUNCE));
                            if ok {
                                // Lock-free dedup against in-app handler.
                                let ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as u64;
                                let prev = LAST_DISPATCH_TS.swap(ms, Ordering::Relaxed);
                                if ms.wrapping_sub(prev) > 300 {
                                    if let Some(cb) = m.action_callback.as_ref() {
                                        cb(&action);
                                    }
                                    m.last_action_time = Some(now);
                                    m.wait_for_key_release = true;
                                }
                            }
                        }
                    }
                }
            } else if is_up {
                inner.lock().wait_for_key_release = false;
            }
        }
    }
    CallNextHookEx(HHOOK(std::ptr::null_mut()), n_code, w_param, l_param)
}

#[cfg(not(windows))]
impl HotkeyManager {
    pub fn start(&self) -> Result<()> { Err(anyhow!("Hotkey system is only supported on Windows")) }
    pub fn stop(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_parse_hotkey_single_key() {
        let parsed = parse_hotkey("home").unwrap();
        assert_eq!(parsed.vk_code, 0x24); assert_eq!(parsed.modifiers, Modifiers::default());
    }
    #[test] fn test_parse_hotkey_with_modifier() {
        let parsed = parse_hotkey("alt+1").unwrap();
        assert_eq!(parsed.vk_code, 0x31); assert!(parsed.modifiers.alt);
    }
    #[test] fn test_parse_hotkey_multiple_modifiers() {
        let parsed = parse_hotkey("ctrl+shift+z").unwrap();
        assert_eq!(parsed.vk_code, 0x5A); assert!(parsed.modifiers.ctrl); assert!(parsed.modifiers.shift);
    }
    #[test] fn test_is_action_allowed() {
        assert!(is_action_allowed("toggle_reader", true));
        assert!(!is_action_allowed("volume_up", false));
        assert!(is_action_allowed("open_settings", false));
    }
    #[test] fn test_key_name_to_vk() {
        assert_eq!(key_name_to_vk("a").unwrap(), 0x41);
        assert_eq!(key_name_to_vk("home").unwrap(), 0x24);
        assert_eq!(key_name_to_vk("f1").unwrap(), 0x70);
    }
}
