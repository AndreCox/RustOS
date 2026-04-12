#![no_std]
#![no_main]

use core::cmp::min;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// No longer a single static constant for the file path
// // Filename is now dynamic, passed via _start or defaulting to /PICO.TXT
const MAX_BUFFER: usize = 32768;
const VIEW_LINES: usize = 28;

const SYS_PRINT_CHAR: u64 = 1;
const SYS_EXIT: u64 = 2;
const SYS_CLEAR_SCREEN: u64 = 3;
const SYS_SET_CURSOR: u64 = 4;
const SYS_FS_READ: u64 = 5;
const SYS_FS_WRITE: u64 = 6;

const SYS_YIELD: u64 = 8;
const SYS_GET_KEY: u64 = 9;

#[derive(Clone, Copy)]
enum Event {
    Char(u8),
    Enter,
    Backspace,
    Tab,
    Save,
    Exit,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Delete,
}

struct Editor {
    buffer: [u8; MAX_BUFFER],
    len: usize,
    cursor: usize,
    dirty: bool,
    exit_requested: bool,
    status: [u8; 96],
    status_len: usize,
    filename: [u8; 64],
    filename_len: usize,
    is_prompting: bool,
    prompt_buf: [u8; 64],
    prompt_len: usize,
}

impl Editor {
    const fn new() -> Self {
        Self {
            buffer: [0; MAX_BUFFER],
            len: 0,
            cursor: 0,
            dirty: false,
            exit_requested: false,
            status: [0; 96],
            status_len: 0,
            filename: [0; 64],
            filename_len: 0,
            is_prompting: false,
            prompt_buf: [0; 64],
            prompt_len: 0,
        }
    }

    fn set_filename(&mut self, path: *const u8) {
        if path.is_null() {
            return;
        }
        let mut i = 0;
        unsafe {
            while *path.add(i) != 0 && i < 63 {
                self.filename[i] = *path.add(i);
                i += 1;
            }
        }
        self.filename[i] = 0;
        self.filename_len = i;
    }

    fn set_status(&mut self, msg: &str) {
        let bytes = msg.as_bytes();
        let count = min(bytes.len(), self.status.len());
        self.status[..count].copy_from_slice(&bytes[..count]);
        self.status_len = count;
    }



    fn load_file(&mut self) {
        let read = syscall_fs_read(
            self.filename.as_ptr(),
            self.buffer.as_mut_ptr(),
            MAX_BUFFER as u64,
        );
        if read == u64::MAX {
            self.len = 0;
            self.cursor = 0;
            self.dirty = false;
            self.set_status("New file");
            return;
        }

        self.len = min(read as usize, MAX_BUFFER);
        self.cursor = 0;
        self.dirty = false;
        self.cursor = 0;
        self.dirty = false;
        self.set_status("Loaded file");
    }

    fn save_file(&mut self) {
        let written = syscall_fs_write(
            self.filename.as_ptr(),
            self.buffer.as_ptr(),
            self.len as u64,
        );
        if written == u64::MAX {
            self.set_status("Save failed");
        } else {
            self.dirty = false;
            self.set_status("Saved file");
        }
    }

    fn insert_byte(&mut self, byte: u8) {
        if self.len >= MAX_BUFFER - 1 {
            self.set_status("Buffer full");
            return;
        }

        if self.cursor > self.len {
            self.cursor = self.len;
        }

        let start = self.cursor;
        for idx in (start..self.len).rev() {
            self.buffer[idx + 1] = self.buffer[idx];
        }
        self.buffer[start] = byte;
        self.len += 1;
        self.cursor += 1;
        self.dirty = true;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let remove_at = self.cursor - 1;
        for idx in remove_at..(self.len - 1) {
            self.buffer[idx] = self.buffer[idx + 1];
        }
        self.len -= 1;
        self.cursor -= 1;
        self.dirty = true;
    }

    fn delete_at_cursor(&mut self) {
        if self.cursor >= self.len {
            return;
        }
        for idx in self.cursor..(self.len - 1) {
            self.buffer[idx] = self.buffer[idx + 1];
        }
        self.len -= 1;
        self.dirty = true;
    }

    fn line_start(&self, pos: usize) -> usize {
        let mut idx = min(pos, self.len);
        while idx > 0 && self.buffer[idx - 1] != b'\n' {
            idx -= 1;
        }
        idx
    }

    fn line_end_from(&self, start: usize) -> usize {
        let mut idx = min(start, self.len);
        while idx < self.len && self.buffer[idx] != b'\n' {
            idx += 1;
        }
        idx
    }

    fn line_number_at(&self, pos: usize) -> usize {
        let mut line = 0usize;
        for idx in 0..min(pos, self.len) {
            if self.buffer[idx] == b'\n' {
                line += 1;
            }
        }
        line
    }

    fn column_at(&self, pos: usize) -> usize {
        pos.saturating_sub(self.line_start(pos))
    }

    fn cursor_line_col(&self) -> (usize, usize) {
        (
            self.line_number_at(self.cursor),
            self.column_at(self.cursor),
        )
    }

    fn line_start_for_line(&self, target_line: usize) -> usize {
        if target_line == 0 {
            return 0;
        }

        let mut line = 0usize;
        let mut idx = 0usize;
        while idx < self.len {
            if self.buffer[idx] == b'\n' {
                line += 1;
                if line == target_line {
                    return idx + 1;
                }
            }
            idx += 1;
        }
        self.len
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_right(&mut self) {
        if self.cursor < self.len {
            self.cursor += 1;
        }
    }

    fn move_home(&mut self) {
        self.cursor = self.line_start(self.cursor);
    }

    fn move_end(&mut self) {
        self.cursor = self.line_end_from(self.line_start(self.cursor));
    }

    fn move_up(&mut self) {
        let current_start = self.line_start(self.cursor);
        if current_start == 0 {
            return;
        }

        let col = self.cursor - current_start;
        let previous_end = current_start - 1;
        let previous_start = self.line_start(previous_end);
        let previous_len = previous_end.saturating_sub(previous_start);
        self.cursor = previous_start + min(col, previous_len);
    }

    fn move_down(&mut self) {
        let current_start = self.line_start(self.cursor);
        let current_end = self.line_end_from(current_start);
        if current_end >= self.len {
            return;
        }

        let col = self.cursor - current_start;
        let next_start = current_end + 1;
        let next_end = self.line_end_from(next_start);
        let next_len = next_end.saturating_sub(next_start);
        self.cursor = next_start + min(col, next_len);
    }

    fn handle_event(&mut self, event: Event) -> bool {
        if self.is_prompting {
            return self.handle_prompt_event(event);
        }

        // Reset exit_requested unless the event is Exit
        if !matches!(event, Event::Exit) {
            self.exit_requested = false;
        }

        match event {
            Event::Char(byte) => {
                self.insert_byte(byte);
                true
            }
            Event::Enter => {
                self.insert_byte(b'\n');
                true
            }
            Event::Backspace => {
                self.backspace();
                true
            }
            Event::Tab => {
                self.insert_byte(b' ');
                self.insert_byte(b' ');
                self.insert_byte(b' ');
                self.insert_byte(b' ');
                true
            }
            Event::Save => {
                if self.filename_len == 0 {
                    self.is_prompting = true;
                    self.prompt_len = 0;
                    return true;
                }
                self.save_file();
                true
            }
            Event::Exit => {
                if self.dirty && !self.exit_requested {
                    self.exit_requested = true;
                    self.set_status("Unsaved changes! Press Ctrl-X again to force exit");
                    return true; // Redraw to show the new status
                }
                syscall_clear_screen();
                syscall_exit();
            }
            Event::Up => {
                self.move_up();
                true
            }
            Event::Down => {
                self.move_down();
                true
            }
            Event::Left => {
                self.move_left();
                true
            }
            Event::Right => {
                self.move_right();
                true
            }
            Event::Home => {
                self.move_home();
                true
            }
            Event::End => {
                self.move_end();
                true
            }
            Event::Delete => {
                self.delete_at_cursor();
                true
            }
        }
    }

    fn handle_prompt_event(&mut self, event: Event) -> bool {
        match event {
            Event::Char(byte) => {
                if self.prompt_len < 63 {
                    self.prompt_buf[self.prompt_len] = byte;
                    self.prompt_len += 1;
                }
                true
            }
            Event::Backspace => {
                if self.prompt_len > 0 {
                    self.prompt_len -= 1;
                }
                true
            }
            Event::Enter => {
                if self.prompt_len > 0 {
                    let mut start = 0;
                    if self.prompt_buf[0] != b'/' {
                        self.filename[0] = b'/';
                        start = 1;
                    }
                    for i in 0..self.prompt_len {
                        self.filename[i + start] = self.prompt_buf[i];
                    }
                    self.filename[self.prompt_len + start] = 0;
                    self.filename_len = self.prompt_len + start;
                    self.is_prompting = false;
                    self.save_file();
                } else {
                    self.is_prompting = false;
                    self.set_status("Save cancelled");
                }
                true
            }
            Event::Exit => {
                self.is_prompting = false;
                self.set_status("Save cancelled");
                true
            }
            _ => false,
        }
    }

    fn view_start_line(&self) -> usize {
        let cursor_line = self.line_number_at(self.cursor);
        cursor_line.saturating_sub(VIEW_LINES / 2)
    }

    fn render(&self) {
        syscall_clear_screen();

        if self.is_prompting {
            print_str("Save As: /");
            for &byte in &self.prompt_buf[..self.prompt_len] {
                print_char(byte);
            }
            print_char(b'\n');
            print_char(b'\n'); // Status line space
        } else {
            let (line, col) = self.cursor_line_col();
            let state = if self.dirty { "modified" } else { "saved" };

            print_str("pico | ");
            if self.filename_len > 0 {
                for &byte in &self.filename[..self.filename_len] {
                    print_char(byte);
                }
            } else {
                print_str("[New File]");
            }
            print_str(" | ");
            print_str(state);
            print_str(" | line ");
            print_usize(line + 1);
            print_str(", col ");
            print_usize(col + 1);
            print_str(" | Ctrl-S save, Ctrl-X exit");
            print_char(b'\n');

            if self.status_len > 0 {
                for &byte in &self.status[..self.status_len] {
                    print_char(byte);
                }
                print_char(b'\n');
            } else {
                print_char(b'\n');
            }
        }

        let start_line = self.view_start_line();
        let end_line = start_line + VIEW_LINES;

        let mut line_idx = start_line;
        while line_idx < end_line {
            let start = self.line_start_for_line(line_idx);
            if start >= self.len && line_idx > self.line_number_at(self.len) {
                break;
            }
            let end = self.line_end_from(start);
            for &byte in &self.buffer[start..end] {
                print_char(byte);
            }
            print_char(b'\n');
            line_idx += 1;
        }

        if self.is_prompting {
            syscall_set_cursor(9 + self.prompt_len, 0);
        } else {
            let (line, col) = self.cursor_line_col();
            syscall_set_cursor(col, 2 + line.saturating_sub(start_line));
        }
    }
}

static mut EDITOR: Editor = Editor::new();

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.start")]
pub extern "C" fn _start(arg_ptr: u64) -> ! {
    let editor_ptr = core::ptr::addr_of_mut!(EDITOR);
    unsafe {
        if arg_ptr != 0 {
            (*editor_ptr).set_filename(arg_ptr as *const u8);
        } else {
            // No default filename anymore
            (*editor_ptr).filename_len = 0;
        }
        (*editor_ptr).load_file();
        (*editor_ptr).render();
    }

    loop {
        let key = syscall_get_key();
        if key != 0 {
            if let Some(event) = decode_key(key) {
                let needs_redraw = unsafe { (*editor_ptr).handle_event(event) };
                if needs_redraw {
                    unsafe {
                        (*editor_ptr).render();
                    }
                }
            }
        }
        syscall_yield();
    }
}

fn decode_key(key: u8) -> Option<Event> {
    match key {
        0 => None,
        b'\n' => Some(Event::Enter),
        b'\x08' => Some(Event::Backspace),
        b'\t' => Some(Event::Tab),
        0x13 => Some(Event::Save),
        0x18 => Some(Event::Exit),
        0x80 => Some(Event::Up),
        0x81 => Some(Event::Down),
        0x82 => Some(Event::Left),
        0x83 => Some(Event::Right),
        0x84 => Some(Event::Home),
        0x85 => Some(Event::End),
        0x86 => Some(Event::Delete),
        0x20..=0x7e => Some(Event::Char(key)),
        _ => None,
    }
}

fn print_char(byte: u8) {
    unsafe {
        core::arch::asm!(
            "int 0x80",
            in("rax") SYS_PRINT_CHAR,
            in("rdi") byte as u64,
            options(nostack, preserves_flags)
        );
    }
}

fn syscall_set_cursor(x: usize, y: usize) {
    let arg1 = (x & 0xFFFF) | ((y & 0xFFFF) << 16);
    unsafe {
        core::arch::asm!(
            "int 0x80",
            in("rax") SYS_SET_CURSOR,
            in("rdi") arg1 as u64,
            options(nostack, preserves_flags)
        );
    }
}

fn print_str(s: &str) {
    for &byte in s.as_bytes() {
        print_char(byte);
    }
}

fn print_usize(mut value: usize) {
    let mut digits = [0u8; 20];
    let mut len = 0usize;

    if value == 0 {
        print_char(b'0');
        return;
    }

    while value > 0 {
        digits[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
    }

    while len > 0 {
        len -= 1;
        print_char(digits[len]);
    }
}

fn syscall_clear_screen() {
    unsafe {
        core::arch::asm!(
            "int 0x80",
            in("rax") SYS_CLEAR_SCREEN,
            options(nostack, preserves_flags)
        );
    }
}



fn syscall_get_key() -> u8 {
    let mut result = SYS_GET_KEY;
    unsafe {
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") result,
            options(nostack, preserves_flags)
        );
    }
    result as u8
}

fn syscall_yield() {
    unsafe {
        core::arch::asm!(
            "int 0x80",
            in("rax") SYS_YIELD,
            options(nostack, preserves_flags)
        );
    }
}

fn syscall_exit() -> ! {
    unsafe {
        core::arch::asm!(
            "int 0x80",
            in("rax") SYS_EXIT,
            options(noreturn, nostack)
        );
    }
}

fn syscall_fs_read(path: *const u8, out_buf: *mut u8, len: u64) -> u64 {
    let mut result = SYS_FS_READ;
    unsafe {
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") path as u64,
            in("rsi") out_buf as u64,
            in("rdx") len,
            options(nostack, preserves_flags)
        );
    }
    result
}

fn syscall_fs_write(path: *const u8, input_buf: *const u8, len: u64) -> u64 {
    let mut result = SYS_FS_WRITE;
    unsafe {
        core::arch::asm!(
            "int 0x80",
            inlateout("rax") result,
            in("rdi") path as u64,
            in("rsi") input_buf as u64,
            in("rdx") len,
            options(nostack, preserves_flags)
        );
    }
    result
}
