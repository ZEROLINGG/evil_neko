// lib/src/utils/sys/mod.rs
#![allow(non_snake_case)]
use crate::runtime::*;

#[cfg(windows)]
use crate::utils::win::resolve;

pub mod win;
pub mod info;
pub mod fingerprint;

use std::env as std_env;
use std::sync::LazyLock;

pub static HOME: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("HOME"))
        .or_else(|_| std_env::var(ss!("USERPROFILE")))
        .unwrap_or_else(|_| "/".to_string()).into()
});

pub static APPDATA: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("APPDATA"))
        .unwrap_or_else(|_| s_add!(HOME.as_str(), r"\AppData\Roaming").into_string())
});

pub static LOCALAPPDATA: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("LOCALAPPDATA"))
        .unwrap_or_else(|_| s_add!(HOME.as_str(), r"\AppData\Local").into_string())
});

pub static DESKTOP: LazyLock<String> = LazyLock::new(|| {
    s_add!(HOME.as_str(), r"\Desktop").into_string()
});

pub static SYS_DRIVE: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("SystemDrive")).unwrap_or_else(|_| "C:".to_string())
});

pub static WINDIR: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("windir")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\Windows").into_string())
});

pub static PROGRAMDATA: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("ProgramData")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\ProgramData").into_string())
});

pub static PROG_FILES: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("ProgramFiles")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\Program Files").into_string())
});

pub static PROG_FILES_X86: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("ProgramFiles(x86)")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\Program Files (x86)").into_string())
});

pub static SYSTEM32: LazyLock<String> = LazyLock::new(|| {
    s_add!(WINDIR.as_str(), r"\System32").into_string()
});

pub static SYSWOW64: LazyLock<String> = LazyLock::new(|| {
    s_add!(WINDIR.as_str(), r"\SysWOW64").into_string()
});

pub static TEMP: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("TEMP"))
        .or_else(|_| std_env::var(ss!("TMP")))
        .unwrap_or_else(|_| {
            #[cfg(windows)]
            { s_add!(LOCALAPPDATA.as_str(), r"\Temp").to_string() }
            #[cfg(unix)]
            { sss!("/tmp") }
        })
});

pub static USERS: LazyLock<String> = LazyLock::new(|| {
    if cfg!(windows) {
        std_env::var(ss!("PUBLIC")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\Users\Public").into_string())
    } else if cfg!(target_os = "linux") {
        sss!(r"/home")
    } else { sss!("/Users") }
});

#[cfg(windows)]
pub fn get_username_sysapi() -> Option<HeapStr> {
    use std::os::windows::ffi::OsStringExt;

    
    type FnGetUserNameW = unsafe extern "system" fn(lpBuffer: *mut u16, pcbBuffer: *mut u32) -> i32;

    let get_user_name_w: FnGetUserNameW = resolve("advapi32.dll", "GetUserNameW")?;

    let mut buf = vec![0u16; 512];
    let mut size = buf.len() as u32;

    unsafe {
        if get_user_name_w(buf.as_mut_ptr(), &mut size) != 0 && size > 0 {
            let len = (size - 1) as usize;
            let string_val = std::ffi::OsString::from_wide(&buf[..len])
                .to_string_lossy()
                .into_owned();
            return Some(HeapStr::from(string_val));
        }
    }
    None
}

#[cfg(unix)]
pub fn get_username_sysapi() -> Option<HeapStr> {
    use std::ffi::CStr;

    unsafe {
        let uid = libc::getuid();

        let mut pwd: libc::passwd = std::mem::zeroed();
        let mut pwd_result: *mut libc::passwd = std::ptr::null_mut();

        let buf_size = libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX);
        let buf_size = if buf_size == -1 { 1024 } else { buf_size as usize };
        let mut buf: Vec<libc::c_char> = vec![0; buf_size];

        let status = libc::getpwuid_r(
            uid,
            &mut pwd,
            buf.as_mut_ptr(),
            buf.len(),
            &mut pwd_result,
        );

        if status == 0 && !pwd_result.is_null() {
            let name_ptr = (*pwd_result).pw_name;
            if !name_ptr.is_null() {
                if let Ok(s) = CStr::from_ptr(name_ptr).to_str() {
                    return Some(HeapStr::from(s));
                }
            }
        }
    }
    None
}

#[cfg(not(any(windows, unix)))]
pub fn get_username_sysapi() -> Option<HeapStr> {
    None
}


#[cfg(windows)]
pub fn get_hostname_sysapi() -> Option<HeapStr> {
    use std::os::windows::ffi::OsStringExt;

    type FnGetComputerNameW = unsafe extern "system" fn(lpBuffer: *mut u16, lpnSize: *mut u32) -> i32;

    let get_computer_name_w: FnGetComputerNameW = resolve("kernel32.dll", "GetComputerNameW")?;

    let mut buf = vec![0u16; 256];
    let mut size = buf.len() as u32;

    unsafe {
        if get_computer_name_w(buf.as_mut_ptr(), &mut size) != 0 && size > 0 {
            let string_val = std::ffi::OsString::from_wide(&buf[..size as usize])
                .to_string_lossy()
                .into_owned();
            return Some(HeapStr::from(string_val));
        }
    }
    None
}

#[cfg(unix)]
pub fn get_hostname_sysapi() -> Option<HeapStr> {
    let mut buf = vec![0u8; 256];
    unsafe {
        if libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) == 0 {
            if let Some(pos) = buf.iter().position(|&x| x == 0) {
                if let Ok(s) = std::str::from_utf8(&buf[..pos]) {
                    return Some(HeapStr::from(s));
                }
            }
        }
    }
    None
}

#[cfg(not(any(windows, unix)))]
pub fn get_hostname_sysapi() -> Option<HeapStr> {
    None
}


#[cfg(windows)]
pub fn hide_file_windows(path: &std::path::Path) {
    use std::os::windows::ffi::OsStrExt;
    type FnSetFileAttributesW = unsafe extern "system" fn(*const u16, u32) -> i32;

    // FILE_ATTRIBUTE_HIDDEN (2) | FILE_ATTRIBUTE_SYSTEM (4) = 6
    const FILE_ATTRIBUTE_HIDDEN_SYSTEM: u32 = 0x06;

    if let Some(set_attr) = resolve::<FnSetFileAttributesW>("kernel32.dll", "SetFileAttributesW") {
        let path_w: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        unsafe {
            set_attr(path_w.as_ptr(), FILE_ATTRIBUTE_HIDDEN_SYSTEM);
        }
    }
}