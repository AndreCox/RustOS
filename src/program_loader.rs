use crate::alloc::vec::Vec;
use crate::fs;
use crate::multitasker::scheduler::SCHEDULER;
use crate::multitasker::task::Task;
use crate::println;
use simple_fatfs::FileSystem;

pub fn launch_program(filename: &str) -> Result<u64, &'static str> {
    crate::serial_println!("launch_program: starting for {}", filename);

    let mut fs_lock = fs::FILESYSTEM.lock();
    crate::serial_println!("launch_program: acquired FS lock");
    let fs = fs_lock.as_mut().ok_or("Filesystem not initialized")?;

    let mut actual_path = None;
    if let Ok(dir_iter) = fs.read_dir("/") {
        crate::serial_println!("launch_program: successfully read root dir");
        let target = filename
            .to_uppercase()
            .replace(".", "")
            .replace("\\", "")
            .replace("/", "");
        for entry_result in dir_iter {
            if let Ok(entry) = entry_result {
                if entry.is_file() {
                    let p_stripped = crate::alloc::format!("{}", entry.path())
                        .to_uppercase()
                        .replace(".", "")
                        .replace("\\", "")
                        .replace("/", "");
                    if p_stripped == target || p_stripped == crate::alloc::format!("{}BIN", target)
                    {
                        actual_path = Some(crate::alloc::format!("{}", entry.path()));
                        break;
                    }
                }
            }
        }
    }

    let actual_path = actual_path.ok_or("File not found on FAT32 filesystem")?;
    crate::serial_println!("launch_program: found file at path {}", actual_path);

    let mut file = match fs.get_ro_file(actual_path.as_str()) {
        Ok(f) => f,
        Err(_) => return Err("Failed to open found file"),
    };
    crate::serial_println!("launch_program: successfully opened file");

    // Note: To match simple-fatfs, we might read chunk by chunk or get length.
    // Actually, simple-fatfs provides the file size and stream reading.
    let size = file.file_size() as usize;
    crate::serial_println!("launch_program: file size is {}", size);
    if size == 0 {
        return Err("File is empty");
    }

    let mut file_content: Vec<u8> = alloc::vec::Vec::with_capacity(size);
    let mut buf = [0u8; 512];
    let mut total_read = 0;

    crate::serial_println!("launch_program: starting chunked read loop");
    loop {
        match embedded_io::Read::read(&mut file, &mut buf) {
            Ok(0) => break,
            Ok(n) => {
                file_content.extend_from_slice(&buf[..n]);
                total_read += n;
                if total_read >= size {
                    break;
                }
            }
            Err(_) => return Err("Error reading file"),
        }
    }
    crate::serial_println!(
        "launch_program: finished read loop. Read {} bytes",
        total_read
    );

    // Allocate memory for the program to live forever (or until task is reaped, but we don't handle freeing yet)
    let memory_slice = file_content.leak();
    let entry_point = memory_slice.as_ptr() as u64;
    crate::serial_println!("launch_program: allocated memory at {:#x}", entry_point);

    // Generate a task ID (just a hacky static counter for now)
    static mut NEXT_TASK_ID: u64 = 100;
    let task_id = unsafe {
        let id: u64 = NEXT_TASK_ID;
        NEXT_TASK_ID += 1;
        id
    };

    let new_task = Task::new(task_id, entry_point);
    crate::serial_println!("launch_program: created task");


    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut scheduler_lock = SCHEDULER.lock();
        if let Some(ref mut scheduler) = *scheduler_lock {
            scheduler.add_task(new_task);
        } else {
            return Err("Scheduler not initialized");
        }
        Ok(())
    })?;
    crate::serial_println!("launch_program: task added to scheduler");

    crate::serial_println!("launch_program: complete! Returning task ID.");
    Ok(task_id)
}
