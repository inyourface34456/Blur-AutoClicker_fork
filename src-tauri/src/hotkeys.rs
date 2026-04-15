use crate::AppHandle;
use crate::ClickerState;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::Manager;
#[cfg(target_family = "windows")]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

#[cfg(target_family = "unix")]
use crate::windows_conts::*;
#[cfg(target_family = "unix")]
use device_query::{DeviceQuery, DeviceState};

use crate::engine::worker::now_epoch_ms;
use crate::engine::worker::start_clicker_inner;
use crate::engine::worker::stop_clicker_inner;
use crate::engine::worker::toggle_clicker_inner;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
    pub main_vk: i32,
    pub key_token: String,
}

pub fn register_hotkey_inner(app: &AppHandle, hotkey: String) -> Result<String, String> {
    let binding = parse_hotkey_binding(&hotkey)?;
    let state = app.state::<ClickerState>();
    state
        .suppress_hotkey_until_ms
        .store(now_epoch_ms().saturating_add(250), Ordering::SeqCst);
    state
        .suppress_hotkey_until_release
        .store(true, Ordering::SeqCst);
    *state.registered_hotkey.lock().unwrap() = Some(binding.clone());

    Ok(format_hotkey_binding(&binding))
}

pub fn normalize_hotkey(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .replace("control", "ctrl")
        .replace("command", "super")
        .replace("meta", "super")
        .replace("win", "super")
}

pub fn parse_hotkey_binding(hotkey: &str) -> Result<HotkeyBinding, String> {
    let normalized = normalize_hotkey(hotkey);
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut super_key = false;
    let mut main_key: Option<(i32, String)> = None;

    for token in normalized.split('+').map(str::trim) {
        if token.is_empty() {
            return Err(format!("Invalid hotkey '{hotkey}': found empty key token"));
        }

        match token {
            "alt" | "option" => alt = true,
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "super" | "command" | "cmd" | "meta" | "win" => super_key = true,
            _ => {
                if main_key
                    .replace(parse_hotkey_main_key(token, hotkey)?)
                    .is_some()
                {
                    return Err(format!(
                        "Invalid hotkey '{hotkey}': use modifiers first and only one main key"
                    ));
                }
            }
        }
    }

    let (main_vk, key_token) =
        main_key.ok_or_else(|| format!("Invalid hotkey '{hotkey}': missing main key"))?;

    Ok(HotkeyBinding {
        ctrl,
        alt,
        shift,
        super_key,
        main_vk,
        key_token,
    })
}

pub fn parse_hotkey_main_key(token: &str, original_hotkey: &str) -> Result<(i32, String), String> {
    let lower = token.trim().to_lowercase();

    let mapped = match lower.as_str() {
        "<" | ">" | "intlbackslash" | "oem102" | "nonusbackslash" => {
            Some((VK_OEM_102 as i32, String::from("IntlBackslash")))
        }
        "space" | "spacebar" => Some((VK_SPACE as i32, String::from("space"))),
        "tab" => Some((VK_TAB as i32, String::from("tab"))),
        "enter" => Some((VK_RETURN as i32, String::from("enter"))),
        "backspace" => Some((VK_BACK as i32, String::from("backspace"))),
        "delete" => Some((VK_DELETE as i32, String::from("delete"))),
        "insert" => Some((VK_INSERT as i32, String::from("insert"))),
        "home" => Some((VK_HOME as i32, String::from("home"))),
        "end" => Some((VK_END as i32, String::from("end"))),
        "pageup" => Some((VK_PRIOR as i32, String::from("pageup"))),
        "pagedown" => Some((VK_NEXT as i32, String::from("pagedown"))),
        "up" => Some((VK_UP as i32, String::from("up"))),
        "down" => Some((VK_DOWN as i32, String::from("down"))),
        "left" => Some((VK_LEFT as i32, String::from("left"))),
        "right" => Some((VK_RIGHT as i32, String::from("right"))),
        "esc" | "escape" => Some((VK_ESCAPE as i32, String::from("escape"))),
        "/" | "slash" => Some((VK_OEM_2 as i32, String::from("/"))),
        "\\" | "backslash" => Some((VK_OEM_5 as i32, String::from("\\"))),
        ";" | "semicolon" => Some((VK_OEM_1 as i32, String::from(";"))),
        "'" | "quote" => Some((VK_OEM_7 as i32, String::from("'"))),
        "[" | "bracketleft" => Some((VK_OEM_4 as i32, String::from("["))),
        "]" | "bracketright" => Some((VK_OEM_6 as i32, String::from("]"))),
        "-" | "minus" => Some((VK_OEM_MINUS as i32, String::from("-"))),
        "=" | "equal" => Some((VK_OEM_PLUS as i32, String::from("="))),
        "`" | "backquote" => Some((VK_OEM_3 as i32, String::from("`"))),
        "," | "comma" => Some((VK_OEM_COMMA as i32, String::from(","))),
        "." | "period" => Some((VK_OEM_PERIOD as i32, String::from("."))),
        _ => None,
    };

    if let Some(binding) = mapped {
        return Ok(binding);
    }

    if lower.starts_with('f') && lower.len() <= 3 {
        if let Ok(number) = lower[1..].parse::<i32>() {
            let vk = match number {
                1..=24 => VK_F1 as i32 + (number - 1),
                _ => -1,
            };
            if vk >= 0 {
                return Ok((vk, lower));
            }
        }
    }

    if let Some(letter) = lower.strip_prefix("key") {
        if letter.len() == 1 {
            return parse_hotkey_main_key(letter, original_hotkey);
        }
    }

    if let Some(digit) = lower.strip_prefix("digit") {
        if digit.len() == 1 {
            return parse_hotkey_main_key(digit, original_hotkey);
        }
    }

    if lower.len() == 1 {
        let ch = lower.as_bytes()[0];
        if ch.is_ascii_lowercase() {
            return Ok((ch.to_ascii_uppercase() as i32, lower));
        }
        if ch.is_ascii_digit() {
            return Ok((ch as i32, lower));
        }
    }

    Err(format!(
        "Couldn't recognize '{token}' as a valid key in '{original_hotkey}'"
    ))
}

pub fn format_hotkey_binding(binding: &HotkeyBinding) -> String {
    let mut parts: Vec<String> = Vec::new();

    if binding.ctrl {
        parts.push(String::from("ctrl"));
    }
    if binding.alt {
        parts.push(String::from("alt"));
    }
    if binding.shift {
        parts.push(String::from("shift"));
    }
    if binding.super_key {
        parts.push(String::from("super"));
    }

    parts.push(binding.key_token.clone());
    parts.join("+")
}

pub fn start_hotkey_listener(app: AppHandle) {
    std::thread::spawn(move || {
        let mut was_pressed = false;

        loop {
            let binding = {
                let state = app.state::<ClickerState>();
                let binding = state.registered_hotkey.lock().unwrap().clone();
                binding
            };

            let currently_pressed = binding
                .as_ref()
                .map(is_hotkey_binding_pressed)
                .unwrap_or(false);

            let suppress_until = app
                .state::<ClickerState>()
                .suppress_hotkey_until_ms
                .load(Ordering::SeqCst);
            let suppress_until_release = app
                .state::<ClickerState>()
                .suppress_hotkey_until_release
                .load(Ordering::SeqCst);
            let hotkey_capture_active = app
                .state::<ClickerState>()
                .hotkey_capture_active
                .load(Ordering::SeqCst);

            if hotkey_capture_active {
                was_pressed = currently_pressed;
                std::thread::sleep(Duration::from_millis(12));
                continue;
            }

            if suppress_until_release {
                if currently_pressed {
                    was_pressed = true;
                    std::thread::sleep(Duration::from_millis(12));
                    continue;
                }

                app.state::<ClickerState>()
                    .suppress_hotkey_until_release
                    .store(false, Ordering::SeqCst);
                was_pressed = false;
                std::thread::sleep(Duration::from_millis(12));
                continue;
            }

            if now_epoch_ms() < suppress_until {
                was_pressed = currently_pressed;
                std::thread::sleep(Duration::from_millis(12));
                continue;
            }

            if currently_pressed && !was_pressed {
                handle_hotkey_pressed(&app);
            } else if !currently_pressed && was_pressed {
                handle_hotkey_released(&app);
            }

            was_pressed = currently_pressed;
            std::thread::sleep(Duration::from_millis(12));
        }
    });
}

pub fn handle_hotkey_pressed(app: &AppHandle) {
    let mode = {
        let state = app.state::<ClickerState>();
        let mode = state.settings.lock().unwrap().mode.clone();
        mode
    };

    if mode == "Toggle" {
        let _ = toggle_clicker_inner(app);
    } else if mode == "Hold" {
        let _ = start_clicker_inner(app);
    }
}

pub fn handle_hotkey_released(app: &AppHandle) {
    let mode = {
        let state = app.state::<ClickerState>();
        let mode = state.settings.lock().unwrap().mode.clone();
        mode
    };

    if mode == "Hold" {
        let _ = stop_clicker_inner(app, Some(String::from("Stopped from hold hotkey")));
    }
}

pub fn is_hotkey_binding_pressed(binding: &HotkeyBinding) -> bool {
    let ctrl_down = is_vk_down(VK_CONTROL as i32);
    let alt_down = is_vk_down(VK_MENU as i32);
    let shift_down = is_vk_down(VK_SHIFT as i32);
    let super_down = is_vk_down(VK_LWIN as i32) || is_vk_down(VK_RWIN as i32);

    if ctrl_down != binding.ctrl
        || alt_down != binding.alt
        || shift_down != binding.shift
        || super_down != binding.super_key
    {
        return false;
    }

    is_vk_down(binding.main_vk)
}

#[cfg(target_family = "windows")]
pub fn is_vk_down(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

#[cfg(target_family = "unix")]
pub fn is_vk_down(vk: i32) -> bool {
    thread_local! {
        static DEVICE: DeviceState = DeviceState::new();
    }

    let keycodes = vk_to_keycodes(vk);
    if keycodes.is_empty() {
        return false;
    }
    DEVICE.with(|state| {
        let keys = state.get_keys();
        keycodes.iter().any(|k| keys.contains(k))
    })
}

#[cfg(target_family = "unix")]
fn vk_to_keycodes(vk: i32) -> &'static [device_query::Keycode] {
    use device_query::Keycode as K;
    match vk as u16 {
        // Generic modifiers (either side)
        VK_CONTROL => &[K::LControl, K::RControl],
        VK_MENU => &[K::LAlt, K::RAlt],
        VK_SHIFT => &[K::LShift, K::RShift],
        // Specific modifiers
        VK_LCONTROL => &[K::LControl],
        VK_RCONTROL => &[K::RControl],
        VK_LSHIFT => &[K::LShift],
        VK_RSHIFT => &[K::RShift],
        VK_LMENU => &[K::LAlt],
        VK_RMENU => &[K::RAlt],
        VK_LWIN => &[K::LMeta],
        VK_RWIN => &[K::RMeta],
        // Letters A-Z (VK 0x41-0x5A)
        0x41 => &[K::A],
        0x42 => &[K::B],
        0x43 => &[K::C],
        0x44 => &[K::D],
        0x45 => &[K::E],
        0x46 => &[K::F],
        0x47 => &[K::G],
        0x48 => &[K::H],
        0x49 => &[K::I],
        0x4A => &[K::J],
        0x4B => &[K::K],
        0x4C => &[K::L],
        0x4D => &[K::M],
        0x4E => &[K::N],
        0x4F => &[K::O],
        0x50 => &[K::P],
        0x51 => &[K::Q],
        0x52 => &[K::R],
        0x53 => &[K::S],
        0x54 => &[K::T],
        0x55 => &[K::U],
        0x56 => &[K::V],
        0x57 => &[K::W],
        0x58 => &[K::X],
        0x59 => &[K::Y],
        0x5A => &[K::Z],
        // Digits 0-9 (VK 0x30-0x39)
        0x30 => &[K::Key0],
        0x31 => &[K::Key1],
        0x32 => &[K::Key2],
        0x33 => &[K::Key3],
        0x34 => &[K::Key4],
        0x35 => &[K::Key5],
        0x36 => &[K::Key6],
        0x37 => &[K::Key7],
        0x38 => &[K::Key8],
        0x39 => &[K::Key9],
        // F-keys
        VK_F1 => &[K::F1],
        VK_F2 => &[K::F2],
        VK_F3 => &[K::F3],
        VK_F4 => &[K::F4],
        VK_F5 => &[K::F5],
        VK_F6 => &[K::F6],
        VK_F7 => &[K::F7],
        VK_F8 => &[K::F8],
        VK_F9 => &[K::F9],
        VK_F10 => &[K::F10],
        VK_F11 => &[K::F11],
        VK_F12 => &[K::F12],
        VK_F13 => &[K::F13],
        VK_F14 => &[K::F14],
        VK_F15 => &[K::F15],
        VK_F16 => &[K::F16],
        VK_F17 => &[K::F17],
        VK_F18 => &[K::F18],
        VK_F19 => &[K::F19],
        VK_F20 => &[K::F20],
        // Navigation
        VK_SPACE => &[K::Space],
        VK_RETURN => &[K::Enter],
        VK_TAB => &[K::Tab],
        VK_BACK => &[K::Backspace],
        VK_ESCAPE => &[K::Escape],
        VK_DELETE => &[K::Delete],
        VK_INSERT => &[K::Insert],
        VK_HOME => &[K::Home],
        VK_END => &[K::End],
        VK_PRIOR => &[K::PageUp],
        VK_NEXT => &[K::PageDown],
        VK_UP => &[K::Up],
        VK_DOWN => &[K::Down],
        VK_LEFT => &[K::Left],
        VK_RIGHT => &[K::Right],
        VK_CAPITAL => &[K::CapsLock],
        // Punctuation (OEM keys)
        VK_OEM_COMMA => &[K::Comma],
        VK_OEM_PERIOD => &[K::Dot],
        VK_OEM_2 => &[K::Slash],
        VK_OEM_1 => &[K::Semicolon],
        VK_OEM_7 => &[K::Apostrophe],
        VK_OEM_4 => &[K::LeftBracket],
        VK_OEM_6 => &[K::RightBracket],
        VK_OEM_5 => &[K::BackSlash],
        VK_OEM_3 => &[K::Grave],
        VK_OEM_MINUS => &[K::Minus],
        VK_OEM_PLUS => &[K::Equal],
        _ => &[],
    }
}