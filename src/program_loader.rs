use crate::alloc::vec::Vec;
use crate::fs;
use crate::multitasker::scheduler::SCHEDULER;
use crate::multitasker::task::Task;
use crate::println;
use simple_fatfs::FileSystem;

const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const DT_NULL: u64 = 0;
const DT_RELA: u64 = 7;
const DT_RELASZ: u64 = 8;
const DT_RELAENT: u64 = 9;
const R_X86_64_RELATIVE: u32 = 8;
const USER_STACK_TOP: u64 = 0x0000_0000_8000_0000;
const USER_STACK_PAGES: usize = 8;

#[derive(Clone, Copy)]
struct Elf64Header {
    e_entry: u64,
    e_phoff: u64,
    e_phentsize: u16,
    e_phnum: u16,
}

#[derive(Clone, Copy)]
struct Elf64ProgramHeader {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_filesz: u64,
    p_memsz: u64,
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    let slice = bytes.get(offset..end)?;
    Some(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let slice = bytes.get(offset..end)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    let slice = bytes.get(offset..end)?;
    Some(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn parse_elf64_header(bytes: &[u8]) -> Option<Elf64Header> {
    if bytes.len() < 64 || bytes.get(0..4)? != ELF_MAGIC {
        return None;
    }

    let class = *bytes.get(4)?;
    let endian = *bytes.get(5)?;
    if class != 2 || endian != 1 {
        return None;
    }

    Some(Elf64Header {
        e_entry: read_u64(bytes, 24)?,
        e_phoff: read_u64(bytes, 32)?,
        e_phentsize: read_u16(bytes, 54)?,
        e_phnum: read_u16(bytes, 56)?,
    })
}

fn parse_program_header(bytes: &[u8], offset: usize) -> Option<Elf64ProgramHeader> {
    Some(Elf64ProgramHeader {
        p_type: read_u32(bytes, offset)?,
        p_flags: read_u32(bytes, offset + 4)?,
        p_offset: read_u64(bytes, offset + 8)?,
        p_vaddr: read_u64(bytes, offset + 16)?,
        p_filesz: read_u64(bytes, offset + 32)?,
        p_memsz: read_u64(bytes, offset + 40)?,
    })
}

fn load_elf_image(bytes: &[u8]) -> Result<(Vec<u8>, u64), &'static str> {
    let header = parse_elf64_header(bytes).ok_or("Invalid ELF image")?;
    let mut min_vaddr = u64::MAX;
    let mut max_vaddr = 0u64;
    let mut loadable_segments = Vec::new();
    let mut dynamic_header = None;

    for index in 0..header.e_phnum as usize {
        let ph_offset = header.e_phoff as usize + index * header.e_phentsize as usize;
        let program_header =
            parse_program_header(bytes, ph_offset).ok_or("Invalid ELF program header")?;

        match program_header.p_type {
            PT_LOAD => {
                if program_header.p_memsz == 0 {
                    continue;
                }

                min_vaddr = min_vaddr.min(program_header.p_vaddr);
                max_vaddr = max_vaddr.max(program_header.p_vaddr + program_header.p_memsz);
                loadable_segments.push(program_header);
            }
            PT_DYNAMIC => {
                dynamic_header = Some(program_header);
            }
            _ => {}
        }
    }

    if loadable_segments.is_empty() {
        return Err("ELF has no loadable segments");
    }

    let image_size = max_vaddr
        .checked_sub(min_vaddr)
        .ok_or("Invalid ELF memory layout")? as usize;
    let mut image = Vec::with_capacity(image_size);
    image.resize(image_size, 0);

    for segment in loadable_segments {
        let file_start = segment.p_offset as usize;
        let file_end = file_start
            .checked_add(segment.p_filesz as usize)
            .ok_or("ELF segment file range overflow")?;
        let mem_start = segment
            .p_vaddr
            .checked_sub(min_vaddr)
            .ok_or("ELF segment underflow")? as usize;
        let mem_end = mem_start
            .checked_add(segment.p_filesz as usize)
            .ok_or("ELF segment memory range overflow")?;

        let file_slice = bytes
            .get(file_start..file_end)
            .ok_or("ELF segment exceeds file size")?;
        let dest = image
            .get_mut(mem_start..mem_end)
            .ok_or("ELF segment exceeds image size")?;
        dest.copy_from_slice(file_slice);
    }

    if let Some(dynamic) = dynamic_header {
        let dyn_start = dynamic.p_offset as usize;
        let dyn_end = dyn_start
            .checked_add(dynamic.p_filesz as usize)
            .ok_or("ELF dynamic section range overflow")?;
        let dyn_slice = bytes
            .get(dyn_start..dyn_end)
            .ok_or("ELF dynamic section exceeds file size")?;

        let mut rela_addr = 0u64;
        let mut rela_size = 0u64;
        let mut rela_ent = 24u64;

        let mut offset = 0usize;
        while offset + 16 <= dyn_slice.len() {
            let tag = read_u64(dyn_slice, offset).ok_or("Invalid dynamic entry")?;
            let value = read_u64(dyn_slice, offset + 8).ok_or("Invalid dynamic entry")?;
            match tag {
                DT_NULL => break,
                DT_RELA => rela_addr = value,
                DT_RELASZ => rela_size = value,
                DT_RELAENT => rela_ent = value,
                _ => {}
            }
            offset += 16;
        }

        if rela_addr != 0 && rela_size != 0 {
            if rela_ent == 0 || rela_ent % 24 != 0 {
                return Err("Unsupported ELF relocation entry size");
            }

            let base_ptr = image.as_ptr() as usize;
            let mut rel_offset = 0u64;
            while rel_offset < rela_size {
                let rela_offset = rela_addr
                    .checked_add(rel_offset)
                    .ok_or("Relocation range overflow")? as usize;
                let rela_slice = bytes
                    .get(rela_offset..rela_offset + 24)
                    .ok_or("Invalid relocation entry")?;
                let r_offset = read_u64(rela_slice, 0).ok_or("Invalid relocation entry")?;
                let r_info = read_u64(rela_slice, 8).ok_or("Invalid relocation entry")?;
                let r_addend = read_u64(rela_slice, 16).ok_or("Invalid relocation entry")?;

                if (r_info & 0xffff_ffff) as u32 == R_X86_64_RELATIVE {
                    unsafe {
                        let target = (base_ptr + r_offset as usize) as *mut u64;
                        target.write((base_ptr as u64).wrapping_add(r_addend));
                    }
                }

                rel_offset += rela_ent;
            }
        }
    }

    let entry_offset = header
        .e_entry
        .checked_sub(min_vaddr)
        .ok_or("ELF entry is outside the loaded image")?;

    let _ = USER_STACK_PAGES;
    Ok((image, entry_offset))
}

fn load_program_image(bytes: &[u8]) -> Result<(Vec<u8>, u64), &'static str> {
    if let Some(header) = parse_elf64_header(bytes) {
        let mut min_vaddr = u64::MAX;
        let mut max_vaddr = 0u64;
        let mut loadable_segments = Vec::new();

        for index in 0..header.e_phnum as usize {
            let ph_offset = header.e_phoff as usize + index * header.e_phentsize as usize;
            let program_header =
                parse_program_header(bytes, ph_offset).ok_or("Invalid ELF program header")?;

            if program_header.p_type != PT_LOAD || program_header.p_memsz == 0 {
                continue;
            }

            min_vaddr = min_vaddr.min(program_header.p_vaddr);
            max_vaddr = max_vaddr.max(program_header.p_vaddr + program_header.p_memsz);
            loadable_segments.push(program_header);
        }

        if loadable_segments.is_empty() {
            return Err("ELF has no loadable segments");
        }

        let image_size = max_vaddr
            .checked_sub(min_vaddr)
            .ok_or("Invalid ELF memory layout")? as usize;
        let mut image = Vec::with_capacity(image_size);
        image.resize(image_size, 0);

        for segment in loadable_segments {
            let file_start = segment.p_offset as usize;
            let file_end = file_start
                .checked_add(segment.p_filesz as usize)
                .ok_or("ELF segment file range overflow")?;
            let mem_start = segment
                .p_vaddr
                .checked_sub(min_vaddr)
                .ok_or("ELF segment underflow")? as usize;
            let mem_end = mem_start
                .checked_add(segment.p_filesz as usize)
                .ok_or("ELF segment memory range overflow")?;

            let file_slice = bytes
                .get(file_start..file_end)
                .ok_or("ELF segment exceeds file size")?;
            let dest = image
                .get_mut(mem_start..mem_end)
                .ok_or("ELF segment exceeds image size")?;
            dest.copy_from_slice(file_slice);
        }

        let entry_offset = header
            .e_entry
            .checked_sub(min_vaddr)
            .ok_or("ELF entry is outside the loaded image")?;

        return Ok((image, entry_offset));
    }

    Ok((bytes.to_vec(), 0))
}

pub fn launch_program(filename: &str, arg: Option<&str>) -> Result<u64, &'static str> {
    crate::serial_println!("launch_program: starting for {}", filename);

    let file_content = fs::with_filesystem(|fs_slot| -> Result<Vec<u8>, &'static str> {
        crate::serial_println!("launch_program: acquired FS lock");
        let fs = fs_slot.as_mut().ok_or("Filesystem not initialized")?;

        let mut actual_path = None;

        // If a path was provided, try it directly first.
        if filename.contains('/') {
            if fs.get_ro_file(filename).is_ok() {
                actual_path = Some(filename.into());
            } else {
                let with_bin = crate::alloc::format!("{}.bin", filename);
                if fs.get_ro_file(with_bin.as_str()).is_ok() {
                    actual_path = Some(with_bin);
                }
            }
        }

        // Backwards-compatible root lookup by bare program name.
        if actual_path.is_none() {
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
                            if p_stripped == target
                                || p_stripped == crate::alloc::format!("{}BIN", target)
                            {
                                actual_path = Some(crate::alloc::format!("{}", entry.path()));
                                break;
                            }
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

        Ok(file_content)
    })?;

    let (program_image, entry_offset) = load_elf_image(&file_content)?;

    // Allocate memory for the program to live forever (or until task is reaped, but we don't handle freeing yet)
    let memory_slice = program_image.leak();
    let entry_point = memory_slice.as_ptr() as u64 + entry_offset;
    crate::serial_println!("launch_program: allocated memory at {:#x}", entry_point);

    let arg_ptr = if let Some(a) = arg {
        let mut s = crate::alloc::format!("{}\0", a);
        let leaked = s.leak();
        leaked.as_ptr() as u64
    } else {
        0
    };

    // Generate a task ID (just a hacky static counter for now)
    static mut NEXT_TASK_ID: u64 = 100;
    let task_id = unsafe {
        let id: u64 = NEXT_TASK_ID;
        NEXT_TASK_ID += 1;
        id
    };

    let new_task = Task::new(task_id, entry_point, arg_ptr);
    crate::serial_println!("launch_program: created task");

    crate::multitasker::scheduler::with_scheduler(|scheduler_slot| {
        if let Some(ref mut scheduler) = *scheduler_slot {
            // Hand keyboard focus to the new task before interrupts can run it.
            // Otherwise a very short-lived program like `hello` can exit before
            // we switch focus, and the launcher would then point focus at a dead task.
            crate::io::keyboard::set_focus_and_clear(task_id);
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
