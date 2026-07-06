// lib/src/utils/mod.rs
#![allow(unused)]

use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::runtime::*;

#[cfg(windows)]
pub mod win;
pub mod sys;
mod awk;
pub mod tlsh;

#[cfg(windows)]
use win::{resolve, win_fn, win_types};
use crate::utils::sys::info;


#[cfg(target_os = "windows")]
pub fn get_running_processes() -> Vec<String> {
    use std::os::windows::ffi::OsStringExt;

    let mut processes = Vec::new();

    unsafe {
        // 动态解析所有需要的API
        let create_snapshot = match resolve::<win_fn::FnCreateToolhelp32Snapshot>(
            ss!("kernel32.dll"),
            ss!("CreateToolhelp32Snapshot"),
        ) {
            Some(f) => f,
            None => return processes,
        };

        let process_first =
            match resolve::<win_fn::FnProcess32FirstW>(ss!("kernel32.dll"), ss!("Process32FirstW")) {
                Some(f) => f,
                None => return processes,
            };

        let process_next =
            match resolve::<win_fn::FnProcess32NextW>(ss!("kernel32.dll"), ss!("Process32NextW")) {
                Some(f) => f,
                None => return processes,
            };

        let close_handle = match resolve::<win_fn::FnCloseHandle>(ss!("kernel32.dll"), ss!("CloseHandle")) {
            Some(f) => f,
            None => return processes,
        };

        let snapshot = create_snapshot(win_types::toolhelp::TH32CS_SNAPPROCESS, 0);
        if snapshot == win_types::INVALID_HANDLE_VALUE || snapshot.is_null() {
            return processes;
        }

        let mut entry = win_types::toolhelp::ProcessEntry32W {
            dw_size: size_of::<win_types::toolhelp::ProcessEntry32W>() as u32,
            cnt_usage: 0,
            th32_process_id: 0,
            th32_default_heap_id: 0,
            th32_module_id: 0,
            cnt_threads: 0,
            th32_parent_process_id: 0,
            pc_pri_class_base: 0,
            dw_flags: 0,
            sz_exe_file: [0; 260],
        };

        if process_first(snapshot, &mut entry) != 0 {
            loop {
                let name_len = entry
                    .sz_exe_file
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.sz_exe_file.len());

                if name_len > 0 {
                    let name = std::ffi::OsString::from_wide(&entry.sz_exe_file[..name_len])
                        .to_string_lossy()
                        .into_owned()
                        .to_lowercase();
                    processes.push(name);
                }

                entry.dw_size = size_of::<win_types::toolhelp::ProcessEntry32W>() as u32;
                if process_next(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        close_handle(snapshot);
    }

    processes
}

#[cfg(target_os = "windows")]
pub fn get_parent_process() -> Option<String> {
    use std::os::windows::ffi::OsStringExt;

    unsafe {
        let get_current_pid =
            resolve::<win_fn::FnGetCurrentProcessId>(ss!("kernel32.dll"), ss!("GetCurrentProcessId"))?;

        let create_snapshot = resolve::<win_fn::FnCreateToolhelp32Snapshot>(
            ss!("kernel32.dll"),
            ss!("CreateToolhelp32Snapshot"),
        )?;

        let process_first =
            resolve::<win_fn::FnProcess32FirstW>(ss!("kernel32.dll"), ss!("Process32FirstW"))?;

        let process_next = resolve::<win_fn::FnProcess32NextW>(ss!("kernel32.dll"), ss!("Process32NextW"))?;

        let close_handle = resolve::<win_fn::FnCloseHandle>(ss!("kernel32.dll"), ss!("CloseHandle"))?;

        let current_pid = get_current_pid();
        let snapshot = create_snapshot(win_types::toolhelp::TH32CS_SNAPPROCESS, 0);

        if snapshot == win_types::INVALID_HANDLE_VALUE || snapshot.is_null() {
            return None;
        }

        let mut entry = win_types::toolhelp::ProcessEntry32W {
            dw_size: size_of::<win_types::toolhelp::ProcessEntry32W>() as u32,
            ..std::mem::zeroed()
        };

        let mut parent_pid = 0u32;

        // 第一次遍历：找父进程ID
        if process_first(snapshot, &mut entry) != 0 {
            loop {
                if entry.th32_process_id == current_pid {
                    parent_pid = entry.th32_parent_process_id;
                    break;
                }

                entry.dw_size = size_of::<win_types::toolhelp::ProcessEntry32W>() as u32;
                if process_next(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        if parent_pid == 0 {
            close_handle(snapshot);
            return None;
        }

        // 第二次遍历：找父进程名称
        entry.dw_size = size_of::<win_types::toolhelp::ProcessEntry32W>() as u32;
        if process_first(snapshot, &mut entry) != 0 {
            loop {
                if entry.th32_process_id == parent_pid {
                    let name_len = entry
                        .sz_exe_file
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.sz_exe_file.len());

                    let name = std::ffi::OsString::from_wide(&entry.sz_exe_file[..name_len])
                        .to_string_lossy()
                        .into_owned();

                    close_handle(snapshot);
                    return Some(name);
                }

                entry.dw_size = size_of::<win_types::toolhelp::ProcessEntry32W>() as u32;
                if process_next(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        close_handle(snapshot);
        None
    }
}

#[cfg(target_os = "linux")]
pub fn get_parent_process() -> Option<String> {
    use std::process;
    let pid = std::process::id();
    let status = std::fs::read_to_string(format!("/proc/{}/status", pid)).ok()?;

    for line in status.lines() {
        if line.starts_with("PPid:") {
            let ppid = line.split_whitespace().nth(1)?;
            let cmdline = std::fs::read_to_string(format!("/proc/{}/cmdline", ppid)).ok()?;
            return Some(cmdline.replace('\0', " "));
        }
    }
    None
}

#[cfg(target_os = "linux")]
pub fn get_running_processes() -> Vec<String> {
    let mut processes = Vec::new();

    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            if let Ok(file_name) = entry.file_name().into_string() {
                if file_name.chars().all(|c| c.is_ascii_digit()) {
                    let cmdline_path = format!("/proc/{}/cmdline", file_name);
                    if let Ok(cmdline) = std::fs::read_to_string(cmdline_path) {
                        let process_name = cmdline
                            .split('\0')
                            .next()
                            .unwrap_or("")
                            .split('/')
                            .last()
                            .unwrap_or("")
                            .to_lowercase();

                        if !process_name.is_empty() {
                            processes.push(process_name);
                        }
                    }
                }
            }
        }
    }

    processes
}

#[cfg(target_os = "macos")]
pub fn get_parent_process() -> Option<String> {
    use std::process::Command;

    let output = Command::new("ps")
        .args(&["-p", &process::id().to_string(), "-o", "ppid="])
        .output()
        .ok()?;

    let ppid = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let output = Command::new("ps")
        .args(&["-p", &ppid, "-o", "comm="])
        .output()
        .ok()?;

    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(target_os = "macos")]
pub fn get_running_processes() -> Vec<String> {
    use std::process::Command;

    let output = match Command::new("ps").args(&["-ax", "-o", "comm="]).output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim().split('/').last().unwrap_or("").to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}


