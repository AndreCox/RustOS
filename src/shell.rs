use crate::alloc::string::String;
use crate::alloc::vec::Vec;
use crate::fs;
use crate::io::keyboard::{SCANCODE_QUEUE, SHELL_TASK_ID, focused_task, scancode_to_char};
use crate::multitasker::yield_now;
use crate::print;
use crate::println;
use crate::program_loader::launch_program;

pub fn task_shell() -> ! {
    let mut input_buffer = String::new();
    let mut current_dir = String::from("/");
    let mut path_entries: Vec<String> = Vec::new();
    path_entries.push(String::from("/apps"));
    path_entries.push(String::from("/bin"));
    path_entries.push(String::from("/"));

    // Give system time to initialize
    crate::timer::sleep_ms(1000);

    println!("\nWelcome to RustOS Shell!");
    print_prompt(&current_dir);

    loop {
        if focused_task() != SHELL_TASK_ID {
            yield_now();
            continue;
        }

        // Handle input from the keyboard queue
        while let Some(scancode) = SCANCODE_QUEUE.pop() {
            if let Some(character) = scancode_to_char(scancode) {
                if character == '\n' {
                    println!();
                    let cmd = input_buffer.trim();
                    if !cmd.is_empty() {
                        execute_command(cmd, &mut current_dir, &mut path_entries);
                    }
                    input_buffer.clear();
                    print_prompt(&current_dir);
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

fn print_prompt(current_dir: &str) {
    print!("{}> ", current_dir);
}

fn normalize_path(path: &str) -> String {
    let mut parts = Vec::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            if !parts.is_empty() {
                parts.pop();
            }
            continue;
        }
        parts.push(part);
    }

    if parts.is_empty() {
        return String::from("/");
    }

    let mut out = String::from("/");
    out.push_str(parts[0]);
    for part in &parts[1..] {
        out.push('/');
        out.push_str(part);
    }
    out
}

fn resolve_path(current_dir: &str, input: &str) -> String {
    if input.is_empty() {
        return current_dir.into();
    }

    if input.starts_with('/') {
        return normalize_path(input);
    }

    if current_dir == "/" {
        normalize_path(&crate::alloc::format!("/{}", input))
    } else {
        normalize_path(&crate::alloc::format!("{}/{}", current_dir, input))
    }
}

fn join_path(base_dir: &str, item: &str) -> String {
    if base_dir == "/" {
        normalize_path(&crate::alloc::format!("/{}", item))
    } else {
        normalize_path(&crate::alloc::format!("{}/{}", base_dir, item))
    }
}

fn command_candidates(current_dir: &str, cmd: &str, path_entries: &[String]) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();

    if cmd.contains('/') {
        candidates.push(resolve_path(current_dir, cmd));
        return candidates;
    }

    for dir in path_entries {
        let resolved_dir = resolve_path(current_dir, dir.as_str());
        candidates.push(join_path(resolved_dir.as_str(), cmd));
    }

    // Preserve the old behavior as a final fallback.
    candidates.push(resolve_path(current_dir, cmd));
    candidates.push(cmd.into());
    candidates
}

fn execute_command(cmd_line: &str, current_dir: &mut String, path_entries: &mut Vec<String>) {
    let mut parts = cmd_line.split_whitespace();
    let cmd = parts.next().unwrap_or("");

    match cmd {
        "help" => {
            println!("Available commands:");
            println!("  help      - Show this message");
            println!("  clear     - Clear the screen");
            println!("  ls        - List files in current directory");
            println!("  mkdir <name> - Create a directory in the current directory");
            println!("  cd <path> - Change current directory");
            println!("  pwd       - Print current directory");
            println!("  rm <file> - Remove a file");
            println!("  path      - Show executable search path");
            println!("  path add <dir> - Add a search directory");
            println!("  path rm <dir> - Remove a search directory");
            println!("  <program> - Run a .bin program");
        }
        "clear" => {
            // run system call to clear screen and move cursor to top-left
            unsafe {
                core::arch::asm!(
                    "int 0x80",
                    in("rax") 3u64, // System call 3: clear_screen
                    options(nostack, preserves_flags)
                );
                core::arch::asm!(
                    "int 0x80",
                    in("rax") 4u64, // System call 4: move_cursor
                    in("rdi") 0u64, // X coordinate
                    in("rsi") 16u64, // Y coordinate
                    options(nostack, preserves_flags)
                );
            }
        }
        "ls" => {
            let fs_lock = fs::FILESYSTEM.lock();
            if let Some(fs) = fs_lock.as_ref() {
                if let Ok(dir_iter) = fs.read_dir(current_dir.as_str()) {
                    let mut shown_paths: Vec<String> = Vec::new();
                    for entry_result in dir_iter {
                        if let Ok(entry) = entry_result {
                            let path = crate::alloc::format!("{}", entry.path());
                            if shown_paths.iter().any(|p| p == &path) {
                                continue;
                            }
                            shown_paths.push(path.clone());

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
        "mkdir" => {
            if let Some(dir_name) = parts.next() {
                let full_path = resolve_path(current_dir, dir_name);
                let fs_lock = fs::FILESYSTEM.lock();
                if let Some(fs) = fs_lock.as_ref() {
                    match fs.create_dir(full_path.as_str()) {
                        Ok(_) => println!("Directory '{}' created.", full_path),
                        Err(e) => println!("Error creating directory '{}': {:?}", full_path, e),
                    }
                } else {
                    println!("Error: Filesystem not initialized.");
                }
            } else {
                println!("Usage: mkdir <directory_name>");
            }
        }
        "cd" => {
            let target = parts.next().unwrap_or("/");
            let new_dir = resolve_path(current_dir, target);
            let fs_lock = fs::FILESYSTEM.lock();
            if let Some(fs) = fs_lock.as_ref() {
                if fs.read_dir(new_dir.as_str()).is_ok() {
                    *current_dir = new_dir;
                } else {
                    println!("cd: no such directory: {}", target);
                }
            } else {
                println!("Error: Filesystem not initialized.");
            }
        }
        "pwd" => {
            println!("{}", current_dir);
        }
        "rm" => {
            if let Some(filename) = parts.next() {
                let full_path = resolve_path(current_dir, filename);
                let fs_lock = fs::FILESYSTEM.lock();
                if let Some(fs) = fs_lock.as_ref() {
                    match fs.remove_file(full_path.as_str()) {
                        Ok(_) => println!("File '{}' removed.", full_path),
                        Err(e) => println!("Error removing file '{}': {:?}", full_path, e),
                    }
                } else {
                    println!("Error: Filesystem not initialized.");
                }
            } else {
                println!("Usage: rm <filename>");
            }
        }
        "path" => match parts.next() {
            None => {
                println!("PATH={}", path_entries.join(":"));
            }
            Some("add") => {
                if let Some(dir) = parts.next() {
                    let resolved = resolve_path(current_dir, dir);
                    if path_entries.iter().any(|p| p == &resolved) {
                        println!("path: already present: {}", resolved);
                    } else {
                        path_entries.push(resolved.clone());
                        println!("path: added {}", resolved);
                    }
                } else {
                    println!("Usage: path add <dir>");
                }
            }
            Some("rm") => {
                if let Some(dir) = parts.next() {
                    let resolved = resolve_path(current_dir, dir);
                    let old_len = path_entries.len();
                    path_entries.retain(|p| p != &resolved);
                    if path_entries.len() == old_len {
                        println!("path: not found: {}", resolved);
                    } else {
                        println!("path: removed {}", resolved);
                    }
                } else {
                    println!("Usage: path rm <dir>");
                }
            }
            Some(_) => {
                println!("Usage: path [add <dir>|rm <dir>]");
            }
        },
        _ => {
            let arg = parts.next().map(|a| resolve_path(current_dir, a));

            // Let the program loader resolve path/case variants for each PATH candidate.
            let candidates = command_candidates(current_dir, cmd, path_entries);
            for filename in candidates {
                match launch_program(filename.as_str(), arg.as_deref()) {
                    Ok(task_id) => {
                        println!("Launched {} with Task ID {}", filename.as_str(), task_id);
                        return;
                    }
                    Err(_) => {
                        continue;
                    }
                }
            }

            println!("Command not found: {}", cmd);
        }
    }
}
