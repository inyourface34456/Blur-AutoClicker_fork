use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
#[cfg(target_family = "windows")]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
    MOUSEINPUT,
};
#[cfg(target_family = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SetCursorPos, SM_CXSCREEN, SM_CYSCREEN,
};
#[cfg(target_family = "unix")]
use enigo::{Enigo, Settings, Mouse, Button, Direction};
#[cfg(target_family = "unix")]
use winit::event_loop::EventLoop;

pub const LEFTDOWN: u32 = 0x0002;
pub const LEFTUP: u32 = 0x0004;
pub const RIGHTDOWN: u32 = 0x0008;
pub const RIGHTUP: u32 = 0x0010;
pub const MIDDLEDOWN: u32 = 0x0020;
pub const MIDDLEUP: u32 = 0x0040;

use super::rng::SmallRng;
use super::sleep_interruptible;

#[cfg(target_family = "windows")]
pub fn current_cursor_position() -> Option<(i32, i32)> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut point = POINT { x: 0, y: 0 };
    let ok = unsafe { GetCursorPos(&mut point) };
    if ok == 0 {
        None
    } else {
        Some((point.x, point.y))
    }
}
#[cfg(target_family = "unix")]
pub fn current_cursor_position() -> Option<(i32, i32)> {
    let mouse = Enigo::new(&Settings::default()).unwrap();
    mouse.location().ok()
}

pub fn current_screen_size() -> Option<(i32, i32)> {
    #[cfg(target_family = "windows")]
    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    #[cfg(target_family = "windows")]
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    
    #[cfg(target_family = "unix")]
    let (width, height) = {
        let screen = Enigo::new(&Settings::default()).unwrap();
        screen.main_display().unwrap()
    };
    
    if width <= 0 || height <= 0 {
        return None;
    };
    
    #[cfg(target_family = "windows")]
    let dpi = {
        use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
        unsafe { GetDpiForSystem() }
    };
    #[cfg(target_family = "unix")]
    let dpi = {
        let event_loop = EventLoop::new().unwrap();
        let monitor = event_loop.primary_monitor();
    
        match monitor {
            Some(m) => (m.scale_factor() * 96.0) as u32,
            None => 96,
        }
        // 96
    };
    
    let scale = dpi as f64 / 96.0;

    Some((
        (width as f64 / scale) as i32,
        (height as f64 / scale) as i32,
    ))
}

#[inline]
pub fn get_cursor_pos() -> (i32, i32) {
    current_cursor_position().unwrap_or((0, 0))
}

#[inline]
pub fn move_mouse(x: i32, y: i32) {
    #[cfg(target_family = "windows")]
    unsafe { SetCursorPos(x, y) };
    #[cfg(target_family = "unix")]
    let mut mouse = Enigo::new(&Settings::default()).unwrap();
    mouse.move_mouse(x, y, enigo::Coordinate::Abs);
}

#[cfg(target_family = "windows")]
#[inline]
pub fn make_input(flags: u32, time: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time,
                dwExtraInfo: 0,
            },
        },
    }
}

#[cfg(target_family = "unix")]
#[inline]
pub fn make_input(flags: u32, _time: u32) -> (Button, Direction) {
    let button = if flags & (RIGHTDOWN | RIGHTUP) != 0 {
        Button::Right
    } else if flags & (MIDDLEDOWN | MIDDLEUP) != 0 {
        Button::Middle
    } else {
        Button::Left
    };

    let direction = if flags & (LEFTDOWN | RIGHTDOWN | MIDDLEDOWN) != 0 {
        Direction::Press
    } else {
        Direction::Release
    };

    (button, direction)
}

#[cfg(target_family = "windows")]
#[inline]
pub fn send_mouse_event(flags: u32) {
    let input = make_input(flags, 0);
    unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
}

#[cfg(target_family = "unix")]
#[inline]
pub fn send_mouse_event(flags: u32) {
    let (button, direction) = make_input(flags, 0);

    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    enigo.button(button, direction);
}

#[cfg(target_family = "windows")]
pub fn send_batch(down: u32, up: u32, n: usize, _hold_ms: u32) {
    let mut inputs: Vec<INPUT> = Vec::with_capacity(n * 2);
    for _ in 0..n {
        inputs.push(make_input(down, 0));
        inputs.push(make_input(up, 0));
    }
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
}

#[cfg(target_family = "unix")]
pub fn send_batch(down: u32, up: u32, n: usize, _hold_ms: u32) {
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    let (btn_down, dir_down) = make_input(down, 0);
    let (btn_up, dir_up) = make_input(up, 0);

    for _ in 0..n {
        enigo.button(btn_down, dir_down);
        enigo.button(btn_up, dir_up);
    }
}

#[cfg(target_family = "windows")]
#[inline]
pub fn get_button_flags(button: i32) -> (u32, u32) {
    match button {
        2 => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        3 => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
        _ => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
    }
}

#[cfg(target_family = "unix")]
#[inline]
pub fn get_button_flags(button: i32) -> (u32, u32) {
    match button {
        // right click
        2 => (RIGHTDOWN, RIGHTUP),

        // middle click
        3 => (MIDDLEDOWN, MIDDLEUP),

        // default = left click
        _ => (LEFTDOWN, LEFTUP),
    }
}

pub fn send_clicks(
    down: u32,
    up: u32,
    count: usize,
    hold_ms: u32,
    use_double_click_gap: bool,
    double_click_delay_ms: u32,
    running: &Arc<AtomicBool>,
) {
    if count == 0 {
        return;
    }

    if !use_double_click_gap && count > 1 && hold_ms == 0 {
        send_batch(down, up, count, hold_ms);
        return;
    }

    for index in 0..count {
        if !running.load(Ordering::SeqCst) {
            return;
        }

        send_mouse_event(down);
        if hold_ms > 0 {
            sleep_interruptible(Duration::from_millis(hold_ms as u64), running);
        }
        send_mouse_event(up);

        if index + 1 < count && use_double_click_gap && double_click_delay_ms > 0 {
            sleep_interruptible(Duration::from_millis(double_click_delay_ms as u64), running);
        }
    }
}

#[inline]
pub fn ease_in_out_quad(t: f64) -> f64 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

#[inline]
pub fn cubic_bezier(t: f64, p0: f64, p1: f64, p2: f64, p3: f64) -> f64 {
    let u = 1.0 - t;
    u * u * u * p0 + 3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t * p3
}

pub fn smooth_move(
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    duration_ms: u64,
    rng: &mut SmallRng,
) {
    if duration_ms < 5 {
        move_mouse(end_x, end_y);
        return;
    }

    let (sx, sy) = (start_x as f64, start_y as f64);
    let (ex, ey) = (end_x as f64, end_y as f64);
    let (dx, dy) = (ex - sx, ey - sy);
    let distance = (dx * dx + dy * dy).sqrt();
    if distance < 1.0 {
        return;
    }

    let (perp_x, perp_y) = (-dy / distance, dx / distance);
    let sign = |b: bool| if b { 1.0f64 } else { -1.0 };
    let o1 = (rng.next_f64() * 0.3 + 0.15) * distance * sign(rng.next_f64() >= 0.5);
    let o2 = (rng.next_f64() * 0.3 + 0.15) * distance * sign(rng.next_f64() >= 0.5);
    let cp1x = sx + dx * 0.33 + perp_x * o1;
    let cp1y = sy + dy * 0.33 + perp_y * o1;
    let cp2x = sx + dx * 0.66 + perp_x * o2;
    let cp2y = sy + dy * 0.66 + perp_y * o2;

    let steps = (duration_ms as usize).clamp(10, 200);
    let step_dur = Duration::from_millis(duration_ms / steps as u64);

    for i in 0..=steps {
        let t = ease_in_out_quad(i as f64 / steps as f64);
        move_mouse(
            cubic_bezier(t, sx, cp1x, cp2x, ex) as i32,
            cubic_bezier(t, sy, cp1y, cp2y, ey) as i32,
        );
        if i < steps {
            std::thread::sleep(step_dur);
        }
    }
}
