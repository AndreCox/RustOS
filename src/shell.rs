use crate::alloc::string::String;
use crate::alloc::vec::Vec;
use crate::fs;
use crate::io::keyboard::{scancode_to_char, SCANCODE_QUEUE};
use crate::multitasker::yield_now;
use crate::print;
use crate::println;
use crate::program_loader::launch_program;

pub fn task_shell() -> ! {
    let mut input_buffer = String::new();

    // Give system time to initialize
    crate::timer::sleep_ms(1000);

    println!("\nWelcome to RustOS Shell!");
    print!("> ");

    loop {
        // Handle input from the keyboard queue
        while let Some(scancode) = SCANCODE_QUEUE.pop() {
            if let Some(character) = scancode_to_char(scancode) {
                if character == '\n' {
                    println!();
                    let cmd = input_buffer.trim();
                    if !cmd.is_empty() {
                        execute_command(cmd);
                    }
                    input_buffer.clear();
                    print!("> ");
                } else if character == '\x08' {
                    // Backspace
                    if !input_buffer.is_empty() {
                        input_buffer.pop();
                        // Erase character from screen: backspace, space, backspace
                        print!("\x08 \x08");
                    }
                } else {
                    input_buffer.push(character);
                    print!("{}", character);
                }
            }
        }
        
        yield_now();
    }
}

fn execute_command(cmd_line: &str) {
    let mut parts = cmd_line.split_whitespace();
    let cmd = parts.next().unwrap_or("");

    match cmd {
        "help" => {
            println!("Available commands:");
            println!("  help      - Show this message");
            println!("  clear     - Clear the screen");
            println!("  ls        - List files in root directory");
            println!("  <program> - Run a .bin program from the root directory");
        }
        "clear" => {
            // Ideally we'd call a generic clear or reset the writer
            // For now, let's just print a bunch of newlines or call into screen logic.
            // Since we don't have a direct clear function exported nicely, let's fake it
            for _ in 0..50 {
                println!();
            }
        }
        "ls" => {
            let fs_lock = fs::FILESYSTEM.lock();
            if let Some(fs) = fs_lock.as_ref() {
                if let Ok(dir_iter) = fs.read_dir("/") {
                    for entry_result in dir_iter {
                        if let Ok(entry) = entry_result {
                            let path = entry.path();
                            if entry.is_dir() {
                                println!("[DIR]  {}", path);
                            } else if entry.is_file() {
                                let size = entry.file_size();
                                println!("[FILE] {} ({} bytes)", path, size);
                            }
                        }
                    }
                } else {
                    println!("Error: Could not list directory.");
                }
            } else {
                println!("Error: Filesystem not initialized.");
            }
        }
        _ => {
            // Attempt to launch it as a program
            let filename = cmd;

            println!("Attempting to launch {}...", filename);
            match launch_program(filename) {
                Ok(task_id) => {
                    println!("Launched {} with Task ID {}", filename, task_id);
                }
                Err(e) => {
                    println!("Failed to launch {}: {}", filename, e);
                }
            }
        }
    }
}
