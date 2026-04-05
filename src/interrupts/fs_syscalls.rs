/*********************************************************************************************************************************************************************************************************************************************************************************************
 *                                                                                                                                       DOCUMENTATION                                                                                                                                       *
 *                                                                                                                                     SYSCALL MODULES,                                                                                                                                      *
 *                                                                                                             THESE FUNCTIONS ARE BASICALLY INTERUPT HANDLERS FOR THE SYSCALLS.                                                                                                             *
 *                                                                                                  WHEN A PROGRAM IN USERSPACE EXECUTES THE SYSCALL INSTRUCTION, IT TRIGGERS AN INTERRUPT.                                                                                                  *
 * IT WILL THEN RUN THE CORRESPONDING HANDLER IN THIS MODULE, WHICH WILL READ THE SYSCALL NUMBER AND ARGUMENTS FROM THE REGISTERS, PERFORM THE REQUESTED OPERATION (LIKE READING A FILE, WRITING TO A FILE, ETC), AND THEN RETURN THE RESULT BACK TO THE USER PROGRAM THROUGH THE REGISTERS. *
 *********************************************************************************************************************************************************************************************************************************************************************************************/

use alloc::{string::String, vec::Vec};

const SYSCALL_ERR: u64 = u64::MAX;
const MAX_SYSCALL_PATH: usize = 512;
const MAX_SYSCALL_RW: usize = 1024 * 1024;

// Just reads in a cstring and converts it to a rust string.
unsafe fn user_cstr_to_string(ptr: u64, max_len: usize) -> Option<String> {
    if ptr == 0 || max_len == 0 {
        return None;
    }

    let mut bytes = Vec::new();
    let base = ptr as *const u8;
    for i in 0..max_len {
        let b = unsafe { base.add(i).read() };
        if b == 0 {
            break;
        }
        bytes.push(b);
    }

    if bytes.is_empty() {
        return None;
    }

    let s = core::str::from_utf8(&bytes).ok()?;
    Some(s.into())
}

// handler to nomalize paths, mostly to handle things like "./././file.txt"
fn normalize_fs_path(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut i = 0usize;

    while i + 1 < bytes.len() && bytes[i] == b'.' && bytes[i + 1] == b'/' {
        i += 2;
    }

    let mut normalized = String::from("/");
    let mut wrote_any = false;
    let mut last_was_sep = true;

    for &b in &bytes[i..] {
        let c = if b == b'\\' { b'/' } else { b };
        if c == b'/' {
            if !last_was_sep {
                normalized.push('/');
                last_was_sep = true;
            }
            continue;
        }

        normalized.push(c as char);
        wrote_any = true;
        last_was_sep = false;
    }

    if !wrote_any {
        return String::from("/");
    }

    while normalized.len() > 1 && normalized.as_bytes()[normalized.len() - 1] == b'/' {
        normalized.pop();
    }

    normalized
}

// read from the file system into userspace
pub(super) unsafe fn sys_fs_read(path_ptr: u64, buf_ptr: u64, len: u64) -> u64 {
    if buf_ptr == 0 || len == 0 {
        return 0;
    }

    let read_len = core::cmp::min(len as usize, MAX_SYSCALL_RW);
    let path = match unsafe { user_cstr_to_string(path_ptr, MAX_SYSCALL_PATH) } {
        Some(p) => normalize_fs_path(&p),
        None => return SYSCALL_ERR,
    };

    let mut fs_lock = crate::fs::FILESYSTEM.lock();
    let fs = match fs_lock.as_mut() {
        Some(fs) => fs,
        None => return SYSCALL_ERR,
    };

    let mut file = match fs.get_ro_file(path.as_str()) {
        Ok(f) => f,
        Err(_) => return SYSCALL_ERR,
    };

    let out = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, read_len) };
    match embedded_io::Read::read(&mut file, out) {
        Ok(n) => n as u64,
        Err(_) => SYSCALL_ERR,
    }
}

// open a file and return a handle to it
pub(super) unsafe fn sys_fs_open(path_ptr: u64) -> u64 {
    let path = match user_cstr_to_string(path_ptr, MAX_SYSCALL_PATH) {
        Some(p) => normalize_fs_path(&p),
        None => return SYSCALL_ERR,
    };

    let mut fs_lock = crate::fs::FILESYSTEM.lock();
    let fs = match fs_lock.as_mut() {
        Some(fs) => fs,
        None => return SYSCALL_ERR,
    };

    let file = match fs.get_ro_file(path.as_str()) {
        Ok(f) => f,
        Err(_) => return SYSCALL_ERR,
    };

    let mut open_files = crate::fs::OPEN_FILES.lock();
    for (i, slot) in open_files.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(file.props.clone());
            return i as u64;
        }
    }
    open_files.push(Some(file.props.clone()));
    (open_files.len() - 1) as u64
}

// read from an already opened file handle into userspace
pub(super) unsafe fn sys_fs_read_handle(handle: u64, buf_ptr: u64, len: u64) -> u64 {
    let props = {
        let open_files = crate::fs::OPEN_FILES.lock();
        match open_files.get(handle as usize) {
            Some(Some(p)) => p.clone(),
            _ => return SYSCALL_ERR,
        }
    };

    let mut fs_lock = crate::fs::FILESYSTEM.lock();
    let fs = match fs_lock.as_mut() {
        Some(fs) => fs,
        None => return SYSCALL_ERR,
    };

    let mut file = simple_fatfs::ROFile::from_props(props, fs);
    let out = core::slice::from_raw_parts_mut(buf_ptr as *mut u8, len as usize);
    let bytes_read = match embedded_io::Read::read(&mut file, out) {
        Ok(n) => n as u64,
        Err(_) => return SYSCALL_ERR,
    };

    let mut open_files = crate::fs::OPEN_FILES.lock();
    if let Some(slot) = open_files.get_mut(handle as usize) {
        *slot = Some(file.props.clone());
    }

    bytes_read
}

// seek within an already opened file handle, returns new position
pub(super) unsafe fn sys_fs_seek_handle(handle: u64, offset: u64, whence: u64) -> u64 {
    let props = {
        let open_files = crate::fs::OPEN_FILES.lock();
        match open_files.get(handle as usize) {
            Some(Some(p)) => p.clone(),
            _ => return SYSCALL_ERR,
        }
    };

    let mut fs_lock = crate::fs::FILESYSTEM.lock();
    let fs = match fs_lock.as_mut() {
        Some(fs) => fs,
        None => return SYSCALL_ERR,
    };

    let mut file = simple_fatfs::ROFile::from_props(props, fs);
    let seek_from = match whence {
        0 => embedded_io::SeekFrom::Start(offset),
        1 => embedded_io::SeekFrom::Current(offset as i64),
        2 => embedded_io::SeekFrom::End(offset as i64),
        _ => return SYSCALL_ERR,
    };

    let new_pos = match embedded_io::Seek::seek(&mut file, seek_from) {
        Ok(n) => n,
        Err(_) => return SYSCALL_ERR,
    };

    let mut open_files = crate::fs::OPEN_FILES.lock();
    if let Some(slot) = open_files.get_mut(handle as usize) {
        *slot = Some(file.props.clone());
    }

    new_pos
}

// close an already opened file handle
pub(super) unsafe fn sys_fs_close(handle: u64) {
    let mut open_files = crate::fs::OPEN_FILES.lock();
    if let Some(slot) = open_files.get_mut(handle as usize) {
        *slot = None;
    }
}

// create a new directory at the given path, returns 0 on success, or SYSCALL_ERR on failure.
pub(super) unsafe fn sys_fs_mkdir(path_ptr: u64) -> u64 {
    let path = match unsafe { user_cstr_to_string(path_ptr, MAX_SYSCALL_PATH) } {
        Some(p) => normalize_fs_path(&p),
        None => return SYSCALL_ERR,
    };

    let mut fs_lock = crate::fs::FILESYSTEM.lock();
    let fs = match fs_lock.as_mut() {
        Some(fs) => fs,
        None => return SYSCALL_ERR,
    };

    if fs.read_dir(path.as_str()).is_ok() {
        return 0;
    }

    match fs.create_dir(path.as_str()) {
        Ok(()) | Err(simple_fatfs::FSError::AlreadyExists) => 0,
        Err(_) => SYSCALL_ERR,
    }
}

// remove a file or directory at the given path, returns 0 on success, or SYSCALL_ERR on failure.
pub(super) unsafe fn sys_fs_remove(path_ptr: u64) -> u64 {
    let path = match unsafe { user_cstr_to_string(path_ptr, MAX_SYSCALL_PATH) } {
        Some(p) => normalize_fs_path(&p),
        None => return SYSCALL_ERR,
    };

    let mut fs_lock = crate::fs::FILESYSTEM.lock();
    let fs = match fs_lock.as_mut() {
        Some(fs) => fs,
        None => return SYSCALL_ERR,
    };

    match fs.remove_file(path.as_str()) {
        Ok(()) => 0,
        Err(_) => SYSCALL_ERR,
    }
}

// allows renaming/moving a file or directory from one path to another, returns 0 on success, or SYSCALL_ERR on failure.
pub(super) unsafe fn sys_fs_rename(from_ptr: u64, to_ptr: u64) -> u64 {
    let from = match unsafe { user_cstr_to_string(from_ptr, MAX_SYSCALL_PATH) } {
        Some(p) => normalize_fs_path(&p),
        None => return SYSCALL_ERR,
    };
    let to = match unsafe { user_cstr_to_string(to_ptr, MAX_SYSCALL_PATH) } {
        Some(p) => normalize_fs_path(&p),
        None => return SYSCALL_ERR,
    };

    let mut fs_lock = crate::fs::FILESYSTEM.lock();
    let fs = match fs_lock.as_mut() {
        Some(fs) => fs,
        None => return SYSCALL_ERR,
    };

    match fs.rename(from.as_str(), to.as_str()) {
        Ok(()) => 0,
        Err(_) => SYSCALL_ERR,
    }
}

// write to the file system from userspace, creating the file if it doesn't exist. Returns number of bytes written, or SYSCALL_ERR on failure.
pub(super) unsafe fn sys_fs_write(path_ptr: u64, buf_ptr: u64, len: u64) -> u64 {
    if buf_ptr == 0 || len == 0 {
        return 0;
    }

    let write_len = core::cmp::min(len as usize, MAX_SYSCALL_RW);
    let path = match unsafe { user_cstr_to_string(path_ptr, MAX_SYSCALL_PATH) } {
        Some(p) => normalize_fs_path(&p),
        None => return SYSCALL_ERR,
    };

    let input = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, write_len) };

    let mut fs_lock = crate::fs::FILESYSTEM.lock();
    let fs = match fs_lock.as_mut() {
        Some(fs) => fs,
        None => return SYSCALL_ERR,
    };

    if fs.get_ro_file(path.as_str()).is_ok() {
        crate::serial_println!("sys_fs_write: File exists, opening rw...");
        let mut file = match fs.get_rw_file(path.as_str()) {
            Ok(f) => f,
            Err(e) => {
                crate::serial_println!("sys_fs_write: Failed to get_rw_file: {:?}", e);
                return SYSCALL_ERR;
            }
        };

        crate::serial_println!("sys_fs_write: Seeking to start...");
        if embedded_io::Seek::seek(&mut file, embedded_io::SeekFrom::Start(0)).is_err() {
            crate::serial_println!("sys_fs_write: Seek failed");
            return SYSCALL_ERR;
        }

        crate::serial_println!("sys_fs_write: Writing {} bytes...", input.len());
        let n = match embedded_io::Write::write(&mut file, input) {
            Ok(n) => n,
            Err(e) => {
                crate::serial_println!("sys_fs_write: Write failed: {:?}", e);
                return SYSCALL_ERR;
            }
        };

        crate::serial_println!("sys_fs_write: Truncating...");
        if file.truncate().is_err() {
            crate::serial_println!("sys_fs_write: Truncate failed");
        }
        let _ = embedded_io::Write::flush(&mut file);

        drop(file);
        crate::serial_println!("sys_fs_write: Success, wrote {}", n);
        let _ = fs.unmount();
        return n as u64;
    }

    {
        let mut file = match fs.create_file(path.as_str()) {
            Ok(f) => f,
            Err(_) => return SYSCALL_ERR,
        };

        let init_buf = alloc::vec![0u8; 4097];
        if embedded_io::Write::write(&mut file, &init_buf).is_err() {
            return SYSCALL_ERR;
        }
        let _ = embedded_io::Write::flush(&mut file);
    }

    let mut file = match fs.get_rw_file(path.as_str()) {
        Ok(f) => f,
        Err(_) => return SYSCALL_ERR,
    };

    if embedded_io::Seek::seek(&mut file, embedded_io::SeekFrom::Start(0)).is_err() {
        return SYSCALL_ERR;
    }

    let n = match embedded_io::Write::write(&mut file, input) {
        Ok(n) => n,
        Err(_) => return SYSCALL_ERR,
    };

    let _ = file.truncate();
    let _ = embedded_io::Write::flush(&mut file);

    drop(file);
    let _ = fs.unmount();
    n as u64
}
