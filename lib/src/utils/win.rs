#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused)]
#![cfg(windows)]

use std::ffi::{c_void, CString};
use core::fmt;



#[cfg(all(windows, target_arch = "x86"))]
#[macro_export]
macro_rules! winlink {
    ($library:literal $abi:literal $($link_name:literal)? fn $($function:tt)*) => (
        #[link(name = $library, kind = "raw-dylib", modifiers = "+verbatim", import_name_type = "undecorated")]
        extern $abi {
            $(#[link_name=$link_name])?
            pub fn $($function)*;
        }
    )
}

#[cfg(all(windows, not(target_arch = "x86")))]
#[macro_export]
macro_rules! winlink {
    ($library:literal $abi:literal $($link_name:literal)? fn $($function:tt)*) => (
        #[link(name = $library, kind = "raw-dylib", modifiers = "+verbatim")]
        unsafe extern $abi {
            $(#[link_name=$link_name])?
            pub fn $($function)*;
        }
    )
}

// =====================================================================
// 顶层通用基础设施 (GUID, HRESULT)
// =====================================================================

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl GUID {
    pub const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self { data1, data2, data3, data4 }
    }
    pub const ZERO: Self = Self::new(0, 0, 0, [0; 8]);

    pub const fn from_u128(uuid: u128) -> Self {
        Self {
            data1: (uuid >> 96) as u32,
            data2: (uuid >> 80) as u16,
            data3: (uuid >> 64) as u16,
            data4: [
                (uuid >> 56) as u8, (uuid >> 48) as u8, (uuid >> 40) as u8, (uuid >> 32) as u8,
                (uuid >> 24) as u8, (uuid >> 16) as u8, (uuid >> 8)  as u8, uuid as u8,
            ],
        }
    }
}

impl fmt::Debug for GUID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Display::fmt(self, f) }
}

impl fmt::Display for GUID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f, "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.data1, self.data2, self.data3, self.data4[0], self.data4[1],
            self.data4[2], self.data4[3], self.data4[4], self.data4[5], self.data4[6], self.data4[7],
        )
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[must_use]
pub struct HRESULT(pub i32);

impl HRESULT {
    #[inline] pub const fn is_ok(self) -> bool { self.0 >= 0 }
    #[inline] pub const fn is_err(self) -> bool { !self.is_ok() }

    #[inline] #[track_caller] pub fn unwrap(self) { assert!(self.is_ok(), "{}", self); }

    #[inline]
    pub fn ok(self) -> anyhow::Result<()> {
        if self.is_ok() { Ok(()) } else { anyhow::bail!("{}", self); }
    }

    #[inline]
    pub fn map<F, T>(self, op: F) -> anyhow::Result<T> where F: FnOnce() -> T {
        self.ok()?; Ok(op())
    }

    #[inline]
    pub fn and_then<F, T>(self, op: F) -> anyhow::Result<T> where F: FnOnce() -> anyhow::Result<T> {
        self.ok()?; op()
    }

    pub fn message(self) -> String {
        const FORMAT_MESSAGE_IGNORE_INSERTS: u32 = 0x00000200;
        const FORMAT_MESSAGE_FROM_SYSTEM: u32 = 0x00001000;
        winlink!("kernel32.dll" "system"
            fn FormatMessageW(
                dwFlags: u32, lpSource: *const c_void, dwMessageId: u32, dwLanguageId: u32,
                lpBuffer: *mut u16, nSize: u32, Arguments: *const c_void,
            ) -> u32
        );
        let mut buf = [0u16; 512];
        unsafe {
            let len = FormatMessageW(
                FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
                std::ptr::null(), self.0 as u32, 0, buf.as_mut_ptr(), buf.len() as u32, std::ptr::null(),
            );
            if len == 0 { return format!("HRESULT 0x{:08X}", self.0 as u32); }
            String::from_utf16_lossy(&buf[..len as usize]).trim_end_matches(['\r', '\n']).to_owned()
        }
    }

    pub const fn from_win32(error: u32) -> Self {
        Self(if error as i32 <= 0 { error } else { (error & 0x0000_FFFF) | (7 << 16) | 0x8000_0000 } as i32)
    }

    pub const fn from_nt(error: i32) -> Self {
        Self(if error >= 0 { error } else { error | 0x1000_0000 })
    }
}
impl fmt::Display for HRESULT {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = self.message();
        if msg.starts_with("HRESULT 0x") {
            write!(f, "{}", msg)
        } else {
            write!(f, "{} (HRESULT 0x{:08X})", msg, self.0 as u32)
        }
    }
}
impl std::fmt::Debug for HRESULT {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// =====================================================================
// Win32 类型、常量与结构体定义 (按使用范围分组)
// =====================================================================

#[cfg(windows)]
pub mod win_types {
    use std::ffi::c_void;

    pub const INVALID_HANDLE_VALUE: *mut c_void = (-1isize) as *mut c_void;

    // --- 内存管理与保护 (Memory) ---
    pub mod memory {
        use std::ffi::c_void;

        // 内存保护属性常量
        pub const PAGE_NOACCESS: u32 = 0x01;
        pub const PAGE_READONLY: u32 = 0x02;
        pub const PAGE_READWRITE: u32 = 0x04;
        pub const PAGE_WRITECOPY: u32 = 0x08;
        pub const PAGE_EXECUTE: u32 = 0x10;
        pub const PAGE_EXECUTE_READ: u32 = 0x20;
        pub const PAGE_EXECUTE_READWRITE: u32 = 0x40;
        pub const PAGE_EXECUTE_WRITECOPY: u32 = 0x80;
        pub const PAGE_GUARD: u32 = 0x100;
        pub const PAGE_NOCACHE: u32 = 0x200;
        pub const PAGE_WRITECOMBINE: u32 = 0x400;

        // 内存分配与状态常量
        pub const MEM_COMMIT: u32 = 0x00001000;
        pub const MEM_RESERVE: u32 = 0x00002000;
        pub const MEM_RELEASE: u32 = 0x00008000;
        pub const MEM_FREE: u32 = 0x00010000;
        pub const MEM_PRIVATE: u32 = 0x00020000;
        pub const MEM_MAPPED: u32 = 0x00040000;
        pub const MEM_IMAGE: u32 = 0x01000000;

        #[repr(C)]
        #[cfg(target_arch = "x86_64")]
        pub struct MEMORY_BASIC_INFORMATION {
            pub base_address: *mut c_void,
            pub allocation_base: *mut c_void,
            pub allocation_protect: u32,
            pub partition_id: u16,
            pub region_size: usize,
            pub state: u32,
            pub protect: u32,
            pub type_: u32,
        }

        #[repr(C)]
        #[cfg(target_arch = "x86")]
        pub struct MEMORY_BASIC_INFORMATION {
            pub base_address: *mut c_void,
            pub allocation_base: *mut c_void,
            pub allocation_protect: u32,
            pub region_size: usize,
            pub state: u32,
            pub protect: u32,
            pub type_: u32,
        }
    }

    // --- 系统快照进程遍历 (ToolHelp32) ---
    pub mod toolhelp {
        pub const TH32CS_SNAPPROCESS: u32 = 0x00000002;
        pub const TH32CS_SNAPTHREAD: u32 = 0x00000004;

        #[repr(C)]
        pub struct ProcessEntry32W {
            pub dw_size: u32,
            pub cnt_usage: u32,
            pub th32_process_id: u32,
            pub th32_default_heap_id: usize,
            pub th32_module_id: u32,
            pub cnt_threads: u32,
            pub th32_parent_process_id: u32,
            pub pc_pri_class_base: i32,
            pub dw_flags: u32,
            pub sz_exe_file: [u16; 260],
        }

        #[repr(C)]
        pub struct ThreadEntry32 {
            pub dw_size: u32,
            pub cnt_usage: u32,
            pub th32_thread_id: u32,
            pub th32_owner_process_id: u32,
            pub tpri_base: i32,
            pub tpri_delta: i32,
            pub dw_flags: u32,
        }
    }

    // --- 异常与调试 (Exception) ---
    pub mod exception {
        #[cfg(target_arch = "x86_64")]
        pub const CONTEXT_DEBUG_REGISTERS: u32 = 0x100010;
        #[cfg(target_arch = "x86")]
        pub const CONTEXT_DEBUG_REGISTERS: u32 = 0x10010;

        pub const EXCEPTION_CONTINUE_EXECUTION: i32 = -1;
        pub const EXCEPTION_CONTINUE_SEARCH: i32 = 0;
    }

    // --- 注册表 (Registry) ---
    pub mod registry {
        use std::ffi::c_void;

        pub const HKEY_CLASSES_ROOT: *mut c_void = 0x80000000usize as *mut c_void;
        pub const HKEY_CURRENT_USER: *mut c_void = 0x80000001usize as *mut c_void;
        pub const HKEY_LOCAL_MACHINE: *mut c_void = 0x80000002usize as *mut c_void;
        pub const HKEY_USERS: *mut c_void = 0x80000003usize as *mut c_void;

        pub const KEY_READ: u32 = 0x20019;
        pub const KEY_WOW64_64KEY: u32 = 0x0100;
        pub const KEY_WOW64_32KEY: u32 = 0x0200;

        pub const REG_SZ: u32 = 1;
        pub const REG_EXPAND_SZ: u32 = 2;
        pub const REG_BINARY: u32 = 3;
        pub const REG_DWORD: u32 = 4;
        pub const REG_MULTI_SZ: u32 = 7;

        pub const ERROR_SUCCESS: i32 = 0;
        pub const ERROR_NO_MORE_ITEMS: i32 = 259;
    }
}

// =====================================================================
// Win32 动态调用函数指针定义
// =====================================================================

#[cfg(windows)]
pub mod win_fn {
    use std::ffi::c_void;
    use super::GUID;
    use super::win_types::memory::MEMORY_BASIC_INFORMATION;
    use super::win_types::toolhelp::{ProcessEntry32W, ThreadEntry32};

    // --- 基础句柄与库管理 ---
    pub type FnGetCurrentProcessId = unsafe extern "system" fn() -> u32;
    pub type FnGetCurrentProcess = unsafe extern "system" fn() -> *mut c_void;
    pub type FnGetCurrentThread = unsafe extern "system" fn() -> *mut c_void;
    pub type FnCloseHandle = unsafe extern "system" fn(*mut c_void) -> i32;
    pub type FnGetModuleHandleA = unsafe extern "system" fn(*const u8) -> *mut c_void;
    pub type FnLoadLibraryA = unsafe extern "system" fn(*const u8) -> *mut c_void;
    pub type FnGetProcAddress = unsafe extern "system" fn(*mut c_void, *const u8) -> Option<unsafe extern "system" fn() -> isize>;
    pub type FnGetLastError = unsafe extern "system" fn() -> u32;
    pub type FnSetLastError = unsafe extern "system" fn(u32);

    // --- 内存管理 (Memory) ---
    pub type FnVirtualProtect = unsafe extern "system" fn(lpAddress: *mut c_void, dwSize: usize, flNewProtect: u32, lpflOldProtect: *mut u32) -> i32;
    pub type FnVirtualAlloc = unsafe extern "system" fn(lpAddress: *mut c_void, dwSize: usize, flAllocationType: u32, flProtect: u32) -> *mut c_void;
    pub type FnVirtualFree = unsafe extern "system" fn(lpAddress: *mut c_void, dwSize: usize, dwFreeType: u32) -> i32;
    pub type FnVirtualQuery = unsafe extern "system" fn(lpAddress: *const c_void, lpBuffer: *mut MEMORY_BASIC_INFORMATION, dwLength: usize) -> usize;
    pub type FnVirtualProtectEx = unsafe extern "system" fn(hProcess: *mut c_void, lpAddress: *mut c_void, dwSize: usize, flNewProtect: u32, lpflOldProtect: *mut u32) -> i32;
    pub type FnVirtualAllocEx = unsafe extern "system" fn(hProcess: *mut c_void, lpAddress: *mut c_void, dwSize: usize, flAllocationType: u32, flProtect: u32) -> *mut c_void;
    pub type FnVirtualFreeEx = unsafe extern "system" fn(hProcess: *mut c_void, lpAddress: *mut c_void, dwSize: usize, dwFreeType: u32) -> i32;
    pub type FnVirtualQueryEx = unsafe extern "system" fn(hProcess: *mut c_void, lpAddress: *const c_void, lpBuffer: *mut MEMORY_BASIC_INFORMATION, dwLength: usize) -> usize;
    pub type FnReadProcessMemory = unsafe extern "system" fn(hProcess: *mut c_void, lpBaseAddress: *const c_void, lpBuffer: *mut c_void, nSize: usize, lpNumberOfBytesRead: *mut usize) -> i32;
    pub type FnWriteProcessMemory = unsafe extern "system" fn(hProcess: *mut c_void, lpBaseAddress: *mut c_void, lpBuffer: *const c_void, nSize: usize, lpNumberOfBytesWritten: *mut usize) -> i32;

    // --- 系统快照 (ToolHelp32) ---
    pub type FnCreateToolhelp32Snapshot = unsafe extern "system" fn(u32, u32) -> *mut c_void;
    pub type FnProcess32FirstW = unsafe extern "system" fn(*mut c_void, *mut ProcessEntry32W) -> i32;
    pub type FnProcess32NextW = unsafe extern "system" fn(*mut c_void, *mut ProcessEntry32W) -> i32;
    pub type FnThread32First = unsafe extern "system" fn(*mut c_void, *mut ThreadEntry32) -> i32;
    pub type FnThread32Next = unsafe extern "system" fn(*mut c_void, *mut ThreadEntry32) -> i32;

    // --- 调试与异常 (Debug API) ---
    pub type FnIsDebuggerPresent = unsafe extern "system" fn() -> i32;
    pub type FnCheckRemoteDebuggerPresent = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
    pub type FnNtQueryInformationProcess = unsafe extern "system" fn(*mut c_void, u32, *mut c_void, u32, *mut u32) -> i32;
    pub type FnGetThreadContext = unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32;
    pub type FnAddVectoredExceptionHandler = unsafe extern "system" fn(u32, Option<unsafe extern "system" fn(*mut c_void) -> i32>) -> *mut c_void;
    pub type FnRemoveVectoredExceptionHandler = unsafe extern "system" fn(*mut c_void) -> u32;

    // --- COM API ---
    pub type FnCoInitializeEx = unsafe extern "system" fn(*const c_void, u32) -> i32;
    pub type FnCoUninitialize = unsafe extern "system" fn();
    pub type FnCoTaskMemFree = unsafe extern "system" fn(*mut c_void);

    // --- Media Foundation API ---
    pub type FnMFStartup = unsafe extern "system" fn(u32, u32) -> i32;
    pub type FnMFShutdown = unsafe extern "system" fn() -> i32;
    pub type FnMFTEnumEx = unsafe extern "system" fn(guidCategory: GUID, flags: u32, pInputType: *const c_void, pOutputType: *const c_void, pppMFTActivate: *mut *mut *mut c_void, pnumMFTActivate: *mut u32) -> i32;
    pub type FnMFCreateAttributes = unsafe extern "system" fn(*mut *mut c_void, u32) -> i32;
    pub type FnMFCreateMediaType = unsafe extern "system" fn(*mut *mut c_void) -> i32;
    pub type FnMFCreateSinkWriterFromURL = unsafe extern "system" fn(*const u16, *mut c_void, *mut c_void, *mut *mut c_void) -> i32;

    // --- 注册表 (Registry API) ---
    pub type FnRegOpenKeyExW = unsafe extern "system" fn(h_key: *mut c_void, sub_key: *const u16, options: u32, sam_desired: u32, result: *mut *mut c_void) -> i32;
    pub type FnRegQueryValueExW = unsafe extern "system" fn(h_key: *mut c_void, value_name: *const u16, reserved: *mut u32, type_: *mut u32, data: *mut u8, cb_data: *mut u32) -> i32;
    pub type FnRegCloseKey = unsafe extern "system" fn(h_key: *mut c_void) -> i32;
    pub type FnRegEnumValueW = unsafe extern "system" fn(h_key: *mut c_void, index: u32, value_name: *mut u16, cb_value_name: *mut u32, reserved: *mut u32, type_: *mut u32, data: *mut u8, cb_data: *mut u32) -> i32;
}

// =====================================================================
// 统一函数解析接口 (动态加载 API 核心机制)
// =====================================================================

#[cfg(windows)]
pub fn resolve<T>(dll: &str, name: &str) -> Option<T>
where
    T: Sized + Copy,
{
    winlink!("kernel32.dll" "system" fn GetModuleHandleA(lpmodulename : *const u8) -> *mut c_void);
    winlink!("kernel32.dll" "system" fn GetProcAddress(hmodule :  *mut c_void, lpprocname : *const u8) -> *const u8);
    winlink!("kernel32.dll" "system" fn LoadLibraryA(lplibfilename : *const u8) -> *mut c_void);

    if dll.is_empty() || name.is_empty() {
        return None;
    }

    let dll_c = CString::new(dll).ok()?;
    let name_c = CString::new(name).ok()?;

    let module = unsafe {
        let mut h = GetModuleHandleA(dll_c.as_ptr() as _);
        if h.is_null() {
            h = LoadLibraryA(dll_c.as_ptr() as _);
        }
        h
    };

    if module.is_null() {
        return None;
    }

    let proc = unsafe { GetProcAddress(module, name_c.as_ptr() as _) };

    (!proc.is_null()).then(|| unsafe { std::mem::transmute_copy(&proc) })
}


pub mod reg {
    use super::*;
    use super::win_types::registry::*;
    use super::win_fn::*;
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::sync::OnceLock;
    use crate::utils::win::win_types::INVALID_HANDLE_VALUE;

    // =================================================================
    // 动态函数指针缓存 (避免重复 resolve)
    // =================================================================
    struct RegApis {
        open_key_ex: FnRegOpenKeyExW,
        query_value_ex: FnRegQueryValueExW,
        close_key: FnRegCloseKey,
        enum_value: FnRegEnumValueW,
    }

    fn get_reg_apis() -> &'static RegApis {
        static APIS: OnceLock<RegApis> = OnceLock::new();
        APIS.get_or_init(|| {
            RegApis {
                open_key_ex: resolve("advapi32.dll", "RegOpenKeyExW").expect("Failed to resolve RegOpenKeyExW"),
                query_value_ex: resolve("advapi32.dll", "RegQueryValueExW").expect("Failed to resolve RegQueryValueExW"),
                close_key: resolve("advapi32.dll", "RegCloseKey").expect("Failed to resolve RegCloseKey"),
                enum_value: resolve("advapi32.dll", "RegEnumValueW").expect("Failed to resolve RegEnumValueW"),
            }
        })
    }

    // =================================================================
    // 注册表值枚举与便捷方法
    // =================================================================
    #[derive(Debug, Clone, PartialEq)]
    pub enum RegValue {
        String(String),
        ExpandString(String),
        MultiString(Vec<String>),
        Dword(u32),
        Binary(Vec<u8>),
        Unknown(u32, Vec<u8>),
    }

    impl RegValue {
        /// 获取字符串引用 (支持 REG_SZ 和 REG_EXPAND_SZ)
        pub fn as_string(&self) -> Option<&str> {
            match self {
                RegValue::String(s) | RegValue::ExpandString(s) => Some(s.as_str()),
                _ => None,
            }
        }

        /// 获取 DWORD 数值
        pub fn as_dword(&self) -> Option<u32> {
            if let RegValue::Dword(v) = self { Some(*v) } else { None }
        }

        /// 获取二进制数据
        pub fn as_binary(&self) -> Option<&[u8]> {
            if let RegValue::Binary(v) = self { Some(v.as_slice()) } else { None }
        }

        /// 获取多行字符串
        pub fn as_multi_string(&self) -> Option<&[String]> {
            if let RegValue::MultiString(v) = self { Some(v.as_slice()) } else { None }
        }
    }

    // --- 内部辅助：将原始字节转换为 RegValue ---
    fn bytes_to_reg_value(reg_type: u32, data: Vec<u8>) -> RegValue {
        // 辅助函数：将字节解析为 UTF-16 字符串（自动去除结尾的 \0）
        let parse_string = |bytes: &[u8]| -> String {
            let u16_data: Vec<u16> = bytes.chunks_exact(2)
                .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                .take_while(|&c| c != 0) // 截断 \0
                .collect();
            String::from_utf16_lossy(&u16_data)
        };

        match reg_type {
            REG_SZ => RegValue::String(parse_string(&data)),
            REG_EXPAND_SZ => RegValue::ExpandString(parse_string(&data)),
            REG_MULTI_SZ => {
                let u16_data: Vec<u16> = data.chunks_exact(2)
                    .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                    .collect();
                let strings: Vec<String> = u16_data.split(|&c| c == 0) // 按 \0 分割
                    .filter(|s| !s.is_empty())                     // 去除连续 \0 导致的空串
                    .map(|s| String::from_utf16_lossy(s))
                    .collect();
                RegValue::MultiString(strings)
            }
            REG_DWORD => {
                if data.len() >= 4 {
                    let val = u32::from_ne_bytes([data[0], data[1], data[2], data[3]]);
                    RegValue::Dword(val)
                } else {
                    RegValue::Unknown(reg_type, data)
                }
            }
            REG_BINARY => RegValue::Binary(data),
            _ => RegValue::Unknown(reg_type, data),
        }
    }

    // =================================================================
    // 注册表句柄 RAII 封装
    // =================================================================
    pub struct RegKey {
        handle: *mut c_void,
    }

    impl Drop for RegKey {
        fn drop(&mut self) {
            if !self.handle.is_null() && self.handle != INVALID_HANDLE_VALUE {
                unsafe { (get_reg_apis().close_key)(self.handle) };
            }
        }
    }

    impl RegKey {
        /// 打开一个注册表项
        /// root: HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER 等
        /// options: 默认通常传 0
        /// access: 权限如 KEY_READ | KEY_WOW64_64KEY
        pub fn open(root: *mut c_void, subkey: &str, options: u32, access: u32) -> anyhow::Result<Self> {
            let apis = get_reg_apis();

            // 转换为 UTF-16 并追加 \0
            let subkey_w: Vec<u16> = std::ffi::OsStr::new(subkey)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let mut handle: *mut c_void = std::ptr::null_mut();

            let status = unsafe {
                (apis.open_key_ex)(root, subkey_w.as_ptr(), options, access, &mut handle)
            };

            HRESULT::from_win32(status as u32).ok()?;

            Ok(Self { handle })
        }

        /// 查询指定名称的值
        pub fn query_value(&self, name: &str) -> anyhow::Result<RegValue> {
            let apis = get_reg_apis();

            let name_w: Vec<u16> = std::ffi::OsStr::new(name)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let mut data_type = 0u32;
            let mut data_size = 0u32;

            // 第一次调用：获取数据大小
            let status = unsafe {
                (apis.query_value_ex)(
                    self.handle, name_w.as_ptr(), std::ptr::null_mut(),
                    &mut data_type, std::ptr::null_mut(), &mut data_size
                )
            };

            HRESULT::from_win32(status as u32).ok()?;

            // 第二次调用：读取数据
            let mut data = vec![0u8; data_size as usize];
            let status = unsafe {
                (apis.query_value_ex)(
                    self.handle, name_w.as_ptr(), std::ptr::null_mut(),
                    &mut data_type, data.as_mut_ptr(), &mut data_size
                )
            };

            HRESULT::from_win32(status as u32).ok()?;

            data.truncate(data_size as usize);
            Ok(bytes_to_reg_value(data_type, data))
        }

        /// 枚举当前注册表项下的所有值
        pub fn enum_values(&self) -> anyhow::Result<Vec<(String, RegValue)>> {
            let apis = get_reg_apis();
            let mut results = Vec::new();
            let mut index = 0u32;

            loop {
                // 预分配缓冲区
                let mut name_buf = vec![0u16; 16383]; // 注册表值名称最大长度
                let mut name_len = name_buf.len() as u32;

                let mut data_type = 0u32;
                let mut data_buf = vec![0u8; 1024];
                let mut data_len = data_buf.len() as u32;

                let status = unsafe {
                    (apis.enum_value)(
                        self.handle, index, name_buf.as_mut_ptr(), &mut name_len,
                        std::ptr::null_mut(), &mut data_type,
                        data_buf.as_mut_ptr(), &mut data_len
                    )
                };

                // 处理缓冲区太小的情况 (ERROR_MORE_DATA = 234)
                if status == 234 {
                    data_buf.resize(data_len as usize, 0);
                    name_len = name_buf.len() as u32; // 必须重置 name_len
                    let retry_status = unsafe {
                        (apis.enum_value)(
                            self.handle, index, name_buf.as_mut_ptr(), &mut name_len,
                            std::ptr::null_mut(), &mut data_type,
                            data_buf.as_mut_ptr(), &mut data_len
                        )
                    };
                    if retry_status != ERROR_SUCCESS {
                        return anyhow::bail!(HRESULT::from_win32(retry_status as u32));
                    }
                } else if status == ERROR_NO_MORE_ITEMS {
                    break;
                } else if status != ERROR_SUCCESS {
                    return anyhow::bail!(HRESULT::from_win32(status as u32));
                }

                data_buf.truncate(data_len as usize);

                let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                let val = bytes_to_reg_value(data_type, data_buf);

                results.push((name, val));
                index += 1;
            }

            Ok(results)
        }
    }
}