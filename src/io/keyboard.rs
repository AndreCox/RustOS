use crossbeam_queue::ArrayQueue;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SCANCODE_QUEUE: ArrayQueue<u8> = ArrayQueue::new(100);
}

static mut LSHIFT: bool = false;
static mut RSHIFT: bool = false;

fn is_shift() -> bool {
    unsafe { LSHIFT || RSHIFT }
}

pub fn scancode_to_char(scancode: u8) -> Option<char> {
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

        // --- Modifier Key Released (Break) ---
        0xAA => {
            unsafe { LSHIFT = false };
            None
        }
        0xB6 => {
            unsafe { RSHIFT = false };
            None
        }

        0x02 => Some(if is_shift() { '!' } else { '1' }),
        0x03 => Some(if is_shift() { '@' } else { '2' }),
        0x04 => Some(if is_shift() { '#' } else { '3' }),
        0x05 => Some(if is_shift() { '$' } else { '4' }),
        0x06 => Some(if is_shift() { '%' } else { '5' }),
        0x07 => Some(if is_shift() { '^' } else { '6' }),
        0x08 => Some(if is_shift() { '&' } else { '7' }),
        0x09 => Some(if is_shift() { '*' } else { '8' }),
        0x0A => Some(if is_shift() { '(' } else { '9' }),
        0x0B => Some(if is_shift() { ')' } else { '0' }),
        0x0C => Some(if is_shift() { '_' } else { '-' }),
        0x0D => Some(if is_shift() { '+' } else { '=' }),

        0x10 => Some(if is_shift() { 'Q' } else { 'q' }),
        0x11 => Some(if is_shift() { 'W' } else { 'w' }),
        0x12 => Some(if is_shift() { 'E' } else { 'e' }),
        0x13 => Some(if is_shift() { 'R' } else { 'r' }),
        0x14 => Some(if is_shift() { 'T' } else { 't' }),
        0x15 => Some(if is_shift() { 'Y' } else { 'y' }),
        0x16 => Some(if is_shift() { 'U' } else { 'u' }),
        0x17 => Some(if is_shift() { 'I' } else { 'i' }),
        0x18 => Some(if is_shift() { 'O' } else { 'o' }),
        0x19 => Some(if is_shift() { 'P' } else { 'p' }),
        0x1A => Some(if is_shift() { '{' } else { '[' }),
        0x1B => Some(if is_shift() { '}' } else { ']' }),
        0x1C => Some('\n'), // Enter

        0x1E => Some(if is_shift() { 'A' } else { 'a' }),
        0x1F => Some(if is_shift() { 'S' } else { 's' }),
        0x20 => Some(if is_shift() { 'D' } else { 'd' }),
        0x21 => Some(if is_shift() { 'F' } else { 'f' }),
        0x22 => Some(if is_shift() { 'G' } else { 'g' }),
        0x23 => Some(if is_shift() { 'H' } else { 'h' }),
        0x24 => Some(if is_shift() { 'J' } else { 'j' }),
        0x25 => Some(if is_shift() { 'K' } else { 'k' }),
        0x26 => Some(if is_shift() { 'L' } else { 'l' }),
        0x27 => Some(if is_shift() { ':' } else { ';' }),
        0x28 => Some(if is_shift() { '\"' } else { '\'' }),

        0x2C => Some(if is_shift() { 'Z' } else { 'z' }),
        0x2D => Some(if is_shift() { 'X' } else { 'x' }),
        0x2E => Some(if is_shift() { 'C' } else { 'c' }),
        0x2F => Some(if is_shift() { 'V' } else { 'v' }),
        0x30 => Some(if is_shift() { 'B' } else { 'b' }),
        0x31 => Some(if is_shift() { 'N' } else { 'n' }),
        0x32 => Some(if is_shift() { 'M' } else { 'm' }),
        0x33 => Some(if is_shift() { '<' } else { ',' }),
        0x34 => Some(if is_shift() { '>' } else { '.' }),
        0x35 => Some(if is_shift() { '?' } else { '/' }),
        0x29 => Some(if is_shift() { '~' } else { '`' }),
        0x2B => Some(if is_shift() { '|' } else { '\\' }),

        0x39 => Some(' '),    // Spacebar
        0x0F => Some('\t'),   // Tab
        0x0E => Some('\x08'), // Backspace character
        _ => None,
    }
}
