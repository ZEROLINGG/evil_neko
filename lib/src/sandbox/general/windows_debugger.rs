#![cfg(windows)]
#![allow(non_snake_case, non_camel_case_types)]

use crate::sandbox::*;
use crate::utils::win::{resolve, win_fn};
use std::arch::asm;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

// ==================== 类型别名 ====================

pub type NTSTATUS = i32;
pub type CONTEXT_FLAGS = u32;

// ==================== 常量定义 ====================

// 异常代码
pub const EXCEPTION_BREAKPOINT: NTSTATUS = 0x80000003_u32 as _;
pub const EXCEPTION_INVALID_HANDLE: NTSTATUS = 0xC0000008_u32 as _;

// 异常处理返回值
const EXCEPTION_CONTINUE_EXECUTION: i32 = -1;
const EXCEPTION_CONTINUE_SEARCH: i32 = 0;

// 上下文标志
#[cfg(target_arch = "x86_64")]
const CONTEXT_DEBUG_REGISTERS: u32 = 0x100010;
#[cfg(target_arch = "x86")]
const CONTEXT_DEBUG_REGISTERS: u32 = 0x10010;

// NtQueryInformationProcess 查询类型
const PROCESS_DEBUG_PORT: u32 = 7;
const PROCESS_DEBUG_OBJECT_HANDLE: u32 = 30;
const PROCESS_DEBUG_FLAGS: u32 = 31;

// PEB 偏移量
#[cfg(target_arch = "x86_64")]
const PEB_NT_GLOBAL_FLAG_OFFSET: usize = 0xBC;
#[cfg(target_arch = "x86")]
const PEB_NT_GLOBAL_FLAG_OFFSET: usize = 0x68;

// 调试标志
const DEBUG_FLAGS: u32 = 0x70;
const ERROR_INVALID_HANDLE: u32 = 6;

// 期望触发异常的 int3 指令地址
static EXPECTED_INT3_ADDR: AtomicU64 = AtomicU64::new(0);

// 标记本次检测中是否被我们自己的 VEH 捕获到异常
static VEH_EXCEPTION_CAUGHT: AtomicBool = AtomicBool::new(false);

// ==================== 基础结构体 ====================

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct M128A {
    pub Low: u64,
    pub High: i64,
}

// ==================== ARM64 结构体 ====================

#[cfg(target_arch = "aarch64")]
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ARM64_NT_NEON128 {
    pub Low: u64,
    pub High: i64,
}

// ==================== XSAVE_FORMAT ====================

#[cfg(any(
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "x86_64"
))]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct XSAVE_FORMAT {
    pub ControlWord: u16,
    pub StatusWord: u16,
    pub TagWord: u8,
    pub Reserved1: u8,
    pub ErrorOpcode: u16,
    pub ErrorOffset: u32,
    pub ErrorSelector: u16,
    pub Reserved2: u16,
    pub DataOffset: u32,
    pub DataSelector: u16,
    pub Reserved3: u16,
    pub MxCsr: u32,
    pub MxCsr_Mask: u32,
    pub FloatRegisters: [M128A; 8],
    pub XmmRegisters: [M128A; 16],
    pub Reserved4: [u8; 96],
}

#[cfg(any(
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "x86_64"
))]
impl Default for XSAVE_FORMAT {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

// ==================== EXCEPTION_RECORD ====================

#[repr(C)]
#[derive(Clone, Copy)]
pub struct EXCEPTION_RECORD {
    pub ExceptionCode: NTSTATUS,
    pub ExceptionFlags: u32,
    pub ExceptionRecord: *mut EXCEPTION_RECORD,
    pub ExceptionAddress: *mut core::ffi::c_void,
    pub NumberParameters: u32,
    pub ExceptionInformation: [usize; 15],
}

impl Default for EXCEPTION_RECORD {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

// ==================== x86_64 / ARM64EC CONTEXT ====================

#[cfg(any(target_arch = "arm64ec", target_arch = "x86_64"))]
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CONTEXT_0_0 {
    pub Header: [M128A; 2],
    pub Legacy: [M128A; 8],
    pub Xmm0: M128A,
    pub Xmm1: M128A,
    pub Xmm2: M128A,
    pub Xmm3: M128A,
    pub Xmm4: M128A,
    pub Xmm5: M128A,
    pub Xmm6: M128A,
    pub Xmm7: M128A,
    pub Xmm8: M128A,
    pub Xmm9: M128A,
    pub Xmm10: M128A,
    pub Xmm11: M128A,
    pub Xmm12: M128A,
    pub Xmm13: M128A,
    pub Xmm14: M128A,
    pub Xmm15: M128A,
}

#[cfg(any(target_arch = "arm64ec", target_arch = "x86_64"))]
#[repr(C)]
#[derive(Clone, Copy)]
pub union CONTEXT_0 {
    pub FltSave: XSAVE_FORMAT,
    pub Anonymous: CONTEXT_0_0,
}

#[cfg(any(target_arch = "arm64ec", target_arch = "x86_64"))]
impl Default for CONTEXT_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

#[cfg(any(target_arch = "arm64ec", target_arch = "x86_64"))]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CONTEXT {
    pub P1Home: u64,
    pub P2Home: u64,
    pub P3Home: u64,
    pub P4Home: u64,
    pub P5Home: u64,
    pub P6Home: u64,
    pub ContextFlags: CONTEXT_FLAGS,
    pub MxCsr: u32,
    pub SegCs: u16,
    pub SegDs: u16,
    pub SegEs: u16,
    pub SegFs: u16,
    pub SegGs: u16,
    pub SegSs: u16,
    pub EFlags: u32,
    pub Dr0: u64,
    pub Dr1: u64,
    pub Dr2: u64,
    pub Dr3: u64,
    pub Dr6: u64,
    pub Dr7: u64,
    pub Rax: u64,
    pub Rcx: u64,
    pub Rdx: u64,
    pub Rbx: u64,
    pub Rsp: u64,
    pub Rbp: u64,
    pub Rsi: u64,
    pub Rdi: u64,
    pub R8: u64,
    pub R9: u64,
    pub R10: u64,
    pub R11: u64,
    pub R12: u64,
    pub R13: u64,
    pub R14: u64,
    pub R15: u64,
    pub Rip: u64,
    pub Anonymous: CONTEXT_0,
    pub VectorRegister: [M128A; 26],
    pub VectorControl: u64,
    pub DebugControl: u64,
    pub LastBranchToRip: u64,
    pub LastBranchFromRip: u64,
    pub LastExceptionToRip: u64,
    pub LastExceptionFromRip: u64,
}

#[cfg(any(target_arch = "arm64ec", target_arch = "x86_64"))]
impl Default for CONTEXT {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

// ==================== AArch64 CONTEXT ====================

#[cfg(target_arch = "aarch64")]
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CONTEXT_0_0 {
    pub X0: u64,
    pub X1: u64,
    pub X2: u64,
    pub X3: u64,
    pub X4: u64,
    pub X5: u64,
    pub X6: u64,
    pub X7: u64,
    pub X8: u64,
    pub X9: u64,
    pub X10: u64,
    pub X11: u64,
    pub X12: u64,
    pub X13: u64,
    pub X14: u64,
    pub X15: u64,
    pub X16: u64,
    pub X17: u64,
    pub X18: u64,
    pub X19: u64,
    pub X20: u64,
    pub X21: u64,
    pub X22: u64,
    pub X23: u64,
    pub X24: u64,
    pub X25: u64,
    pub X26: u64,
    pub X27: u64,
    pub X28: u64,
    pub Fp: u64,
    pub Lr: u64,
}

#[cfg(target_arch = "aarch64")]
#[repr(C)]
#[derive(Clone, Copy)]
pub union CONTEXT_0 {
    pub Anonymous: CONTEXT_0_0,
    pub X: [u64; 31],
}

#[cfg(target_arch = "aarch64")]
impl Default for CONTEXT_0 {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

#[cfg(target_arch = "aarch64")]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CONTEXT {
    pub ContextFlags: CONTEXT_FLAGS,
    pub Cpsr: u32,
    pub Anonymous: CONTEXT_0,
    pub Sp: u64,
    pub Pc: u64,
    pub V: [ARM64_NT_NEON128; 32],
    pub Fpcr: u32,
    pub Fpsr: u32,
    pub Bcr: [u32; 8],
    pub Bvr: [u64; 8],
    pub Wcr: [u32; 2],
    pub Wvr: [u64; 2],
}

#[cfg(target_arch = "aarch64")]
impl Default for CONTEXT {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

// ==================== EXCEPTION_POINTERS ====================

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct EXCEPTION_POINTERS {
    pub ExceptionRecord: *mut EXCEPTION_RECORD,
    pub ContextRecord: *mut CONTEXT,
}

// ==================== PEB 结构体 ====================

#[repr(C)]
struct PEB {
    reserved1: [u8; 2],
    being_debugged: u8,
    // ... 其他字段可以根据需要保留或省略
}

// ==================== VEH Guard (RAII) ====================

struct VehGuard(*mut std::ffi::c_void);

impl Drop for VehGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            if let Some(remove_veh) = resolve::<win_fn::FnRemoveVectoredExceptionHandler>(
                ss!("kernel32.dll"),
                ss!("RemoveVectoredExceptionHandler"),
            ) {
                unsafe {
                    let _ = remove_veh(self.0);
                }
            }
        }
    }
}

// ==================== PEB 访问 ====================

#[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
unsafe fn get_peb() -> *const PEB {
    let peb: *const PEB;

    #[cfg(target_arch = "x86_64")]
    unsafe {
        asm!(
        "mov {}, gs:[0x60]",
        out(reg) peb,
        options(nostack, preserves_flags)
        );
    }

    #[cfg(target_arch = "x86")]
    unsafe {
        asm!(
        "mov {}, fs:[0x30]",
        out(reg) peb,
        options(nostack, preserves_flags)
        );
    }

    peb
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
unsafe fn get_peb() -> *const PEB {
    std::ptr::null()
}

// ==================== 标准 API 检测 ====================

pub fn is_debugger_present() -> bool {
    resolve::<win_fn::FnIsDebuggerPresent>(ss!("kernel32.dll"), ss!("IsDebuggerPresent"))
        .map_or(false, |f| unsafe { f() != 0 })
}

pub fn check_remote_debugger_present() -> Option<bool> {
    let get_curr_proc =
        resolve::<win_fn::FnGetCurrentProcess>(ss!("kernel32.dll"), ss!("GetCurrentProcess"))?;
    let check_remote = resolve::<win_fn::FnCheckRemoteDebuggerPresent>(
        ss!("kernel32.dll"),
        ss!("CheckRemoteDebuggerPresent"),
    )?;

    unsafe {
        let current_process = get_curr_proc();
        let mut being_debugged: i32 = 0;

        if check_remote(current_process, &mut being_debugged) != 0 {
            Some(being_debugged != 0)
        } else {
            None
        }
    }
}

// ==================== NtQueryInformationProcess 底层检测 ====================

// 辅助函数：简化 NtQueryInformationProcess 调用
unsafe fn query_process_info<T>(info_class: u32) -> Option<T>
where
    T: Default,
{
    let get_curr_proc =
        resolve::<win_fn::FnGetCurrentProcess>(ss!("kernel32.dll"), ss!("GetCurrentProcess"))?;
    let nt_query = resolve::<win_fn::FnNtQueryInformationProcess>(
        ss!("ntdll.dll"),
        ss!("NtQueryInformationProcess"),
    )?;

    unsafe {
        let mut value = T::default();
        let status = nt_query(
            get_curr_proc(),
            info_class,
            &mut value as *mut _ as *mut _,
            core::mem::size_of::<T>() as u32,
            std::ptr::null_mut(),
        );

        if status == 0 { Some(value) } else { None }
    }
}

pub fn check_debug_port() -> Option<bool> {
    unsafe { query_process_info::<usize>(PROCESS_DEBUG_PORT).map(|port| port != 0) }
}

pub fn check_debug_object() -> Option<bool> {
    unsafe {
        query_process_info::<*mut std::ffi::c_void>(PROCESS_DEBUG_OBJECT_HANDLE)
            .map(|handle| !handle.is_null())
    }
}

pub fn check_debug_flags() -> Option<bool> {
    unsafe { query_process_info::<u32>(PROCESS_DEBUG_FLAGS).map(|flags| flags == 0) }
}

// ==================== PEB 直接访问检测 ====================

pub fn check_peb_being_debugged() -> Option<bool> {
    unsafe {
        let peb = get_peb();
        if peb.is_null() {
            return None;
        }
        Some((*peb).being_debugged != 0)
    }
}

pub fn check_peb_nt_global_flag() -> Option<bool> {
    unsafe {
        let peb = get_peb() as *const u8;
        if peb.is_null() {
            return None;
        }

        let nt_global_flag = *(peb.add(PEB_NT_GLOBAL_FLAG_OFFSET) as *const u32);
        Some((nt_global_flag & DEBUG_FLAGS) != 0)
    }
}

// ==================== 硬件断点检测 ====================

pub fn check_hardware_breakpoints() -> Option<bool> {
    let get_curr_thread =
        resolve::<win_fn::FnGetCurrentThread>(ss!("kernel32.dll"), ss!("GetCurrentThread"))?;
    let get_thread_ctx =
        resolve::<win_fn::FnGetThreadContext>(ss!("kernel32.dll"), ss!("GetThreadContext"))?;

    unsafe {
        let mut context = CONTEXT::default();
        context.ContextFlags = CONTEXT_DEBUG_REGISTERS;

        if get_thread_ctx(get_curr_thread(), &mut context as *mut _ as *mut _) != 0 {
            #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
            {
                let has_breakpoint =
                    context.Dr0 != 0 || context.Dr1 != 0 || context.Dr2 != 0 || context.Dr3 != 0;
                Some(has_breakpoint)
            }

            #[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
            Some(false)
        } else {
            None
        }
    }
}

// ==================== 异常检测（INT3 软件断点）====================

unsafe extern "system" fn int3_exception_handler(exception_info_ptr: *mut std::ffi::c_void) -> i32 {
    unsafe {
        if exception_info_ptr.is_null() {
            return EXCEPTION_CONTINUE_SEARCH;
        }

        let exception_info = exception_info_ptr as *mut EXCEPTION_POINTERS;
        let exception_record_ptr = (*exception_info).ExceptionRecord;
        let context_record_ptr = (*exception_info).ContextRecord;

        if exception_record_ptr.is_null() || context_record_ptr.is_null() {
            return EXCEPTION_CONTINUE_SEARCH;
        }

        let exception_record = &*exception_record_ptr;
        if exception_record.ExceptionCode != EXCEPTION_BREAKPOINT {
            return EXCEPTION_CONTINUE_SEARCH;
        }

        let expected_addr = EXPECTED_INT3_ADDR.load(Ordering::SeqCst);
        let actual_addr = exception_record.ExceptionAddress as u64;

        if expected_addr == 0 || actual_addr != expected_addr {
            return EXCEPTION_CONTINUE_SEARCH;
        }

        VEH_EXCEPTION_CAUGHT.store(true, Ordering::SeqCst);

        // 跳过 INT3 指令（1 字节）
        #[cfg(target_arch = "x86_64")]
        {
            (*context_record_ptr).Rip += 1;
        }
        #[cfg(target_arch = "x86")]
        {
            (*context_record_ptr).Eip += 1;
        }

        EXCEPTION_CONTINUE_EXECUTION
    }
}

#[cfg(target_arch = "x86_64")]
pub fn is_debugger_present_via_int3() -> bool {
    static INT3_DETECT_LOCK: Mutex<()> = Mutex::new(());
    let _lock = INT3_DETECT_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    VEH_EXCEPTION_CAUGHT.store(false, Ordering::SeqCst);
    EXPECTED_INT3_ADDR.store(0, Ordering::SeqCst);

    let add_veh = match resolve::<win_fn::FnAddVectoredExceptionHandler>(
        ss!("kernel32.dll"),
        ss!("AddVectoredExceptionHandler"),
    ) {
        Some(f) => f,
        None => return false,
    };

    unsafe {
        let handler = add_veh(1, Some(int3_exception_handler));
        if handler.is_null() {
            return false;
        }
        let _veh_guard = VehGuard(handler);

        let flag_addr = EXPECTED_INT3_ADDR.as_ptr();

        asm!(
        "lea {tmp}, [rip + 2f]",
        "mov qword ptr [{flag_addr}], {tmp}",
        "2:",
        "int3",
        tmp = out(reg) _,
        flag_addr = in(reg) flag_addr,
        options(nostack)
        );

        EXPECTED_INT3_ADDR.store(0, Ordering::SeqCst);
        drop(_veh_guard);

        !VEH_EXCEPTION_CAUGHT.load(Ordering::SeqCst)
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn is_debugger_present_via_int3() -> bool {
    false
}

// ==================== CloseHandle 异常检测 ====================

pub fn check_close_handle_exception() -> Option<bool> {
    static DETECT_LOCK: Mutex<()> = Mutex::new(());
    static EXCEPTION_CAUGHT: AtomicBool = AtomicBool::new(false);

    unsafe extern "system" fn close_handle_exception_handler(
        exception_info_ptr: *mut std::ffi::c_void,
    ) -> i32 {
        unsafe {
            if exception_info_ptr.is_null() {
                return EXCEPTION_CONTINUE_SEARCH;
            }

            let exception_info = exception_info_ptr as *mut EXCEPTION_POINTERS;
            let exception_record_ptr = (*exception_info).ExceptionRecord;

            if exception_record_ptr.is_null() {
                return EXCEPTION_CONTINUE_SEARCH;
            }

            let code = (*exception_record_ptr).ExceptionCode as u32;
            if code == EXCEPTION_INVALID_HANDLE as u32 {
                EXCEPTION_CAUGHT.store(true, Ordering::SeqCst);
                return EXCEPTION_CONTINUE_EXECUTION;
            }

            EXCEPTION_CONTINUE_SEARCH
        }
    }

    let _lock = DETECT_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let add_veh = resolve::<win_fn::FnAddVectoredExceptionHandler>(
        ss!("kernel32.dll"),
        ss!("AddVectoredExceptionHandler"),
    )?;
    let close_handle =
        resolve::<win_fn::FnCloseHandle>(ss!("kernel32.dll"), ss!("CloseHandle"))?;
    let get_last_error =
        resolve::<win_fn::FnGetLastError>(ss!("kernel32.dll"), ss!("GetLastError"))?;
    let set_last_error =
        resolve::<win_fn::FnSetLastError>(ss!("kernel32.dll"), ss!("SetLastError"))?;

    unsafe {
        EXCEPTION_CAUGHT.store(false, Ordering::SeqCst);

        let saved_error = get_last_error();
        set_last_error(0);

        let handler = add_veh(1, Some(close_handle_exception_handler));
        if handler.is_null() {
            set_last_error(saved_error);
            return None;
        }
        let _veh_guard = VehGuard(handler);

        let invalid_handle = 0x1234_usize as *mut std::ffi::c_void;
        let _ = close_handle(invalid_handle);

        let error = get_last_error();
        let caught = EXCEPTION_CAUGHT.load(Ordering::SeqCst);

        drop(_veh_guard);
        set_last_error(saved_error);

        if caught {
            Some(true)
        } else if error == ERROR_INVALID_HANDLE {
            Some(false)
        } else {
            None
        }
    }
}

// ==================== 异步检测接口 ====================

pub async fn check_win_debugger(env: Arc<tokio::sync::Mutex<Environment>>) {
    use crate::action;

    let check = async || {
        tokio::task::spawn_blocking(|| {
            (
                is_debugger_present(),
                check_remote_debugger_present(),
                check_debug_port(),
                check_debug_object(),
                check_debug_flags(),
                check_peb_being_debugged(),
                check_peb_nt_global_flag(),
                check_hardware_breakpoints(),
                is_debugger_present_via_int3(),
                check_close_handle_exception(),
            )
        })
        .await
        .unwrap_or((false, None, None, None, None, None, None, None, false, None))
    };

    let results = check().await;

    let (
        is_dbg_present,
        remote_dbg,
        debug_port,
        debug_object,
        debug_flags,
        peb_dbg,
        peb_nt_global,
        hw_bp,
        int3_caught,
        close_handle_caught,
    ) = results;

    let mut env_lock = env.lock().await;

    // 一致性检测
    if seed() % 10 > 3 && results != check().await {
        env_lock.add(action!(
            AbnormalType::Inconsistent,
            ScoreType::OtherSystemApi,
            s!("[Debugger] The two test results were inconsistent."),
            8,
            0.8
        ));
    }

    macro_rules! report {
        ($cond:expr, $ty:expr, $score:expr, $msg:expr, $weight:expr, $confidence:expr) => {
            if $cond {
                env_lock.add(action!($ty, $score, $msg, $weight, $confidence));
            }
        };
    }

    report!(
        is_dbg_present,
        SoftwareType::Debugger,
        ScoreType::OtherSystemApi,
        s!("[Debugger] kernel32!IsDebuggerPresent returned true"),
        10,
        1.0
    );

    report!(
        remote_dbg == Some(true),
        SoftwareType::Debugger,
        ScoreType::OtherSystemApi,
        s!("[Debugger] kernel32!CheckRemoteDebuggerPresent detected an attached debugger"),
        10,
        1.0
    );

    report!(
        debug_port == Some(true),
        SoftwareType::Debugger,
        ScoreType::Process,
        s!("[Debugger] NtQueryInformationProcess: ProcessDebugPort is non-zero"),
        10,
        1.0
    );

    report!(
        debug_object == Some(true),
        SoftwareType::Debugger,
        ScoreType::Process,
        s!("[Debugger] NtQueryInformationProcess: ProcessDebugObjectHandle is valid"),
        10,
        1.0
    );

    report!(
        debug_flags == Some(true),
        SoftwareType::Debugger,
        ScoreType::Process,
        s!("[Debugger] NtQueryInformationProcess: ProcessDebugFlags indicates debugging"),
        10,
        1.0
    );

    report!(
        peb_dbg == Some(true),
        SoftwareType::Debugger,
        ScoreType::Process,
        s!("[Debugger] PEB.BeingDebugged flag is set to true via direct memory access"),
        10,
        1.0
    );

    report!(
        peb_nt_global == Some(true),
        SoftwareType::Debugger,
        ScoreType::Process,
        s!("[Debugger] PEB.NtGlobalFlag contains heap debugging flags (0x70)"),
        10,
        1.0
    );

    report!(
        hw_bp == Some(true),
        SoftwareType::Debugger,
        ScoreType::Cpu,
        s!("[Debugger] Hardware Breakpoints (DR0-DR3) are active in Thread Context"),
        10,
        1.0
    );

    report!(
        int3_caught,
        SoftwareType::Debugger,
        ScoreType::Process,
        s!("[Debugger] INT3 software breakpoint exception was swallowed by a debugger"),
        10,
        0.9
    );

    report!(
        close_handle_caught == Some(true),
        SoftwareType::Debugger,
        ScoreType::Process,
        s!("[Debugger] EXCEPTION_INVALID_HANDLE was caught, indicates debugger presence"),
        10,
        0.9
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_checks() {
        println!("\n========== Individual Anti-Debug Checks ==========");
        println!("IsDebuggerPresent: {}", is_debugger_present());
        println!(
            "CheckRemoteDebuggerPresent: {:?}",
            check_remote_debugger_present()
        );
        println!("DebugPort: {:?}", check_debug_port());
        println!("DebugObject: {:?}", check_debug_object());
        println!("DebugFlags: {:?}", check_debug_flags());
        println!("PEB BeingDebugged: {:?}", check_peb_being_debugged());
        println!("PEB NtGlobalFlag: {:?}", check_peb_nt_global_flag());
        println!("Hardware Breakpoints: {:?}", check_hardware_breakpoints());
        println!("INT3 Exception: {}", is_debugger_present_via_int3());
        println!(
            "CloseHandle Exception: {:?}",
            check_close_handle_exception()
        );
        println!("==================================================\n");
    }
}
