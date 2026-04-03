use core::sync::atomic::{AtomicU64, Ordering};
use crossbeam_queue::ArrayQueue;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SCANCODE_QUEUE: ArrayQueue<u8> = ArrayQueue::new(100);
}

pub const SHELL_TASK_ID: u64 = 6;
static KEYBOARD_FOCUS: AtomicU64 = AtomicU64::new(SHELL_TASK_ID);

static mut LSHIFT: bool = false;
static mut RSHIFT: bool = false;
static mut LCTRL: bool = false;
static mut RCTRL: bool = false;
static mut KEY_HELD: [bool; 128] = [false; 128];

fn is_shift() -> bool {
    unsafe { LSHIFT || RSHIFT }
}

fn is_ctrl() -> bool {
    unsafe { LCTRL || RCTRL }
}

pub fn focused_task() -> u64 {
    KEYBOARD_FOCUS.load(Ordering::Acquire)
}

pub fn task_has_focus(task_id: u64) -> bool {
    focused_task() == task_id
}

pub fn set_focus(task_id: u64) {
    KEYBOARD_FOCUS.store(task_id, Ordering::Release);
}

pub fn clear_scancodes() {
    while SCANCODE_QUEUE.pop().is_some() {}
}

pub fn reset_state() {
    clear_scancodes();
    unsafe {
        LSHIFT = false;
        RSHIFT = false;
        LCTRL = false;
        RCTRL = false;
        KEY_HELD = [false; 128];
    }
}

pub fn set_focus_and_clear(task_id: u64) {
    reset_state();
    set_focus(task_id);
}

pub fn push_scancode(scancode: u8) {
    if scancode == 0xE0 {
        let _ = SCANCODE_QUEUE.push(scancode);
        return;
    }

    let key = (scancode & 0x7F) as usize;
    let released = (scancode & 0x80) != 0;

    unsafe {
        if released {
            KEY_HELD[key] = false;
            let _ = SCANCODE_QUEUE.push(scancode);
        } else if !KEY_HELD[key] {
            KEY_HELD[key] = true;
            let _ = SCANCODE_QUEUE.push(scancode);
        }
    }
}

pub fn scancode_to_char(scancode: u8) -> Option<char> {
    scancode_to_byte(scancode).and_then(|byte| {
        if byte.is_ascii() {
            Some(byte as char)
        } else {
            None
        }
    })
}

pub fn scancode_to_byte(scancode: u8) -> Option<u8> {
    match scancode {
        // --- Modifier Key Pressed (Make) ---
        0x2A => {
            unsafe { LSHIFT = true };
            None
        }
        0x36 => {
            unsafe { RSHIFT = true };
            None
        }
        0x1D => {
            unsafe { LCTRL = true };
            None
        }

        // --- Modifier Key Released (Break) ---
        0xAA => {
            unsafe { LSHIFT = false };
            None
        }
        0xB6 => {
            unsafe { RSHIFT = false };
            None
        }
        0x9D => {
            unsafe { LCTRL = false };
            None
        }

        0x02 => Some(if is_shift() { b'!' } else { b'1' }),
        0x03 => Some(if is_shift() { b'@' } else { b'2' }),
        0x04 => Some(if is_shift() { b'#' } else { b'3' }),
        0x05 => Some(if is_shift() { b'$' } else { b'4' }),
        0x06 => Some(if is_shift() { b'%' } else { b'5' }),
        0x07 => Some(if is_shift() { b'^' } else { b'6' }),
        0x08 => Some(if is_shift() { b'&' } else { b'7' }),
        0x09 => Some(if is_shift() { b'*' } else { b'8' }),
        0x0A => Some(if is_shift() { b'(' } else { b'9' }),
        0x0B => Some(if is_shift() { b')' } else { b'0' }),
        0x0C => Some(if is_shift() { b'_' } else { b'-' }),
        0x0D => Some(if is_shift() { b'+' } else { b'=' }),

        0x10 => Some(if is_shift() { b'Q' } else { b'q' }),
        0x11 => Some(if is_shift() { b'W' } else { b'w' }),
        0x12 => Some(if is_shift() { b'E' } else { b'e' }),
        0x13 => Some(if is_shift() { b'R' } else { b'r' }),
        0x14 => Some(if is_shift() { b'T' } else { b't' }),
        0x15 => Some(if is_shift() { b'Y' } else { b'y' }),
        0x16 => Some(if is_shift() { b'U' } else { b'u' }),
        0x17 => Some(if is_shift() { b'I' } else { b'i' }),
        0x18 => Some(if is_shift() { b'O' } else { b'o' }),
        0x19 => Some(if is_shift() { b'P' } else { b'p' }),
        0x1A => Some(if is_shift() { b'{' } else { b'[' }),
        0x1B => Some(if is_shift() { b'}' } else { b']' }),
        0x1C => Some(b'\n'),

        0x1E => Some(if is_shift() { b'A' } else { b'a' }),
        0x1F => Some(if is_ctrl() {
            0x13
        } else if is_shift() {
            b'S'
        } else {
            b's'
        }),
        0x20 => Some(if is_shift() { b'D' } else { b'd' }),
        0x21 => Some(if is_shift() { b'F' } else { b'f' }),
        0x22 => Some(if is_shift() { b'G' } else { b'g' }),
        0x23 => Some(if is_shift() { b'H' } else { b'h' }),
        0x24 => Some(if is_shift() { b'J' } else { b'j' }),
        0x25 => Some(if is_shift() { b'K' } else { b'k' }),
        0x26 => Some(if is_shift() { b'L' } else { b'l' }),
        0x27 => Some(if is_shift() { b':' } else { b';' }),
        0x28 => Some(if is_shift() { b'\"' } else { b'\'' }),

        0x2C => Some(if is_shift() { b'Z' } else { b'z' }),
        0x2D => Some(if is_ctrl() {
            0x18
        } else if is_shift() {
            b'X'
        } else {
            b'x'
        }),
        0x2E => Some(if is_shift() { b'C' } else { b'c' }),
        0x2F => Some(if is_shift() { b'V' } else { b'v' }),
        0x30 => Some(if is_shift() { b'B' } else { b'b' }),
        0x31 => Some(if is_shift() { b'N' } else { b'n' }),
        0x32 => Some(if is_shift() { b'M' } else { b'm' }),
        0x33 => Some(if is_shift() { b'<' } else { b',' }),
        0x34 => Some(if is_shift() { b'>' } else { b'.' }),
        0x35 => Some(if is_shift() { b'?' } else { b'/' }),
        0x29 => Some(if is_shift() { b'~' } else { b'`' }),
        0x2B => Some(if is_shift() { b'|' } else { b'\\' }),

        0x39 => Some(b' '),
        0x0F => Some(b'\t'),
        0x0E => Some(b'\x08'),
        _ => None,
    }
}
