// // lib/src/sandbox/fs.rs
// #![allow(unused)]
// #[libpm::rt(s1)]
//
//
// use std::env as std_env;
// use tokio::fs as tokio_fs;
// use anyhow::{anyhow, ensure, Result};
// use std::sync::LazyLock;
// use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
// use libpm::*;
// use crate::sandbox::*;
// use crate::{action};
//
// static HOME: LazyLock<String> = LazyLock::new(|| {
//     std_env::var(ss!("HOME"))
//         .or_else(|_| std_env::var(ss!("USERPROFILE")))
//         .unwrap_or_else(|_| "/".to_string())
// });
//
// static APPDATA: LazyLock<String> = LazyLock::new(|| {
//     std_env::var(ss!("APPDATA"))
//         .unwrap_or_else(|_| s_add!(HOME.as_str(), r"\AppData\Roaming"))
// });
//
// static LOCALAPPDATA: LazyLock<String> = LazyLock::new(|| {
//     std_env::var(ss!("LOCALAPPDATA"))
//         .unwrap_or_else(|_| s_add!(HOME.as_str(), r"\AppData\Local"))
// });
//
// static DESKTOP: LazyLock<String> = LazyLock::new(|| {
//     s_add!(HOME.as_str(), r"\Desktop")
// });
// static SYS_DRIVE: LazyLock<String> = LazyLock::new(|| {
//     std_env::var(ss!("SystemDrive")).unwrap_or_else(|_| "C:".to_string())
// });
//
// static WINDIR: LazyLock<String> = LazyLock::new(|| {
//     std_env::var(ss!("windir")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\Windows"))
// });
//
// static PROGRAMDATA: LazyLock<String> = LazyLock::new(|| {
//     std_env::var(ss!("ProgramData")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\ProgramData"))
// });
//
// static PROG_FILES: LazyLock<String> = LazyLock::new(|| {
//     std_env::var(ss!("ProgramFiles")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\Program Files"))
// });
//
// static PROG_FILES_X86: LazyLock<String> = LazyLock::new(|| {
//     std_env::var(ss!("ProgramFiles(x86)")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\Program Files (x86)"))
// });
//
// static SYSTEM32: LazyLock<String> = LazyLock::new(|| {
//     s_add!(WINDIR.as_str(), r"\System32")
// });
//
// static SYSWOW64: LazyLock<String> = LazyLock::new(|| {
//     s_add!(WINDIR.as_str(), r"\SysWOW64")
// });
//
// static TEMP: LazyLock<String> = LazyLock::new(|| {
//     std_env::var(ss!("TEMP"))
//         .or_else(|_| std_env::var(ss!("TMP")))
//         .unwrap_or_else(|_| {
//             #[cfg(windows)]
//             { s_add!(LOCALAPPDATA.as_str(), r"\Temp") }
//             #[cfg(unix)]
//             { sss!("/tmp") }
//         })
// });
// static USERS: LazyLock<String> = LazyLock::new(|| {
//     if cfg!(windows) {
//         std_env::var(ss!("PUBLIC")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\Users\Public"))
//     } else if cfg!(target_os = "linux") {
//         sss!(r"/home")
//     } else { sss!("/Users") }
// });
//
// async fn chk_dir(actions: &mut Vec<ScoreAction>, path: &str, mut action: ScoreAction) {
//     if let Ok(meta) = tokio_fs::metadata(path).await {
//         if meta.is_dir() {
//             action.set_msg(s_add!("dir found: ", path));
//             actions.push(action);
//         }
//     }
// }
//
// const KB: u64 = 1024;
//
//
// pub async fn check_folders(env: Arc<Mutex<Environment>>) {
//     macro_rules! a {
//         ($action_type:expr, $score:expr, $confidence:expr) => {{
//             action!($action_type, $crate::sandbox::ScoreType::Directory, $score, $confidence)
//         }};
//     }
//
//     let mut actions: Vec<ScoreAction> = Vec::new();
//
//
//     // 通用跨平台目录
//     let dirs: Vec<(String, ScoreAction)> = vec![
//         (s_add!(HOME.as_str(), "/OneDrive"), a!(TrustType::PersonalFiles, 6, 0.90)),
//         (s_add!(HOME.as_str(), "/Dropbox"), a!(TrustType::PersonalFiles, 6, 0.85)),
//         (s_add!(HOME.as_str(), "/.aws"), a!(TrustType::Development, 8, 0.90)),
//         (s_add!(HOME.as_str(), "/.kube"), a!(TrustType::Development, 8, 0.90)),
//         (s_add!(HOME.as_str(), "/.cargo"), a!(TrustType::Development, 5, 0.85)),
//         (s_add!(HOME.as_str(), "/.gradle"), a!(TrustType::Development, 5, 0.80)),
//         (s_add!(HOME.as_str(), "/.m2"), a!(TrustType::Development, 5, 0.80)),
//     ];
//
//     // Windows 专属对抗与痕迹目录
//     let dirs_win: Vec<(String, ScoreAction)> = vec![
//         (sss!(r#"C:\sandbox"#), a!(SandboxType::Unknown, 10, 0.9)),
//         (sss!(r#"C:\Sandbox"#), a!(SandboxType::Unknown, 10, 0.9)),
//         (sss!(r#"C:\sandboxes"#), a!(SandboxType::Unknown, 10, 0.9)),
//         (sss!(r#"C:\cuckoo"#), a!(SandboxType::Cuckoo, 10, 0.9)),
//         (sss!(r#"C:\cape"#), a!(SandboxType::CAPE, 10, 0.9)),
//         (sss!(r#"C:\zenbox"#), a!(SandboxType::Zenbox, 9, 0.9)),
//         (sss!(r#"C:\Program Files\Sandbox"#), a!(SandboxType::Unknown, 5, 0.8)),
//         (sss!(r#"C:\Program Files\JoeSandbox"#), a!(SandboxType::JoeSandbox, 5, 0.8)),
//         (sss!(r#"C:\malware"#), a!(SoftwareType::Analysis, 4, 0.5)),
//         (sss!(r#"C:\samples"#), a!(SoftwareType::Analysis, 4, 0.5)),
//         (sss!(r#"C:\VirusTotal"#), a!(SoftwareType::Security, 4, 0.2)),
//         (s_add!(PROG_FILES.as_str(), r#"\Wireshark"#), a!(SoftwareType::Analysis, 6, 0.8)),
//         (s_add!(LOCALAPPDATA.as_str(), r#"\Fiddler"#), a!(SoftwareType::Analysis, 6, 0.8)),
//         (sss!(r#"C:\tools"#), a!(SoftwareType::Analysis, 1, 0.1)),
//         (s_add!(PROG_FILES.as_str(), r#"\VMware\VMware Tools"#), a!(VirtualMachineType::VMware, 9, 0.9)),
//         (s_add!(PROG_FILES.as_str(), r#"\Oracle\VirtualBox Guest Additions"#), a!(VirtualMachineType::VirtualBox, 9, 0.9)),
//         (sss!(r#"C:\Program Files\CrowdStrike"#), a!(SoftwareType::Security, 8, 0.9)),
//         (sss!(r#"C:\Program Files\SentinelOne"#), a!(SoftwareType::Security, 8, 0.9)),
//         (sss!(r#"C:\ProgramData\Microsoft\Windows Defender\AdvancedThreatProtection"#), a!(SoftwareType::Security, 8, 0.9)),
//         (sss!(r#"C:\Program Files\FireEye\xagt"#), a!(SoftwareType::Security, 8, 0.9)),
//         (s_add!(PROG_FILES.as_str(), r#"\SystemInformer"#), a!(SoftwareType::Analysis, 8, 0.8)),
//         (sss!(r#"C:\fakenet"#), a!(SoftwareType::Analysis, 9, 0.9)),
//         (s_add!(PROG_FILES_X86.as_str(), r#"\Steam\steamapps\common"#), a!(TrustType::Game, 10, 0.95)),
//         (sss!(r#"D:\SteamLibrary\steamapps\common"#), a!(TrustType::Game, 10, 0.85)),
//         (s_add!(PROG_FILES.as_str(), r#"\Epic Games"#), a!(TrustType::Game, 8, 0.90)),
//         (s_add!(PROG_FILES_X86.as_str(), r#"\WeGame"#), a!(TrustType::Game, 8, 0.90)),
//         (sss!(r#"C:\Riot Games"#), a!(TrustType::Game, 8, 0.85)),
//         (s_add!(HOME.as_str(), r#"\Documents\WeChat Files"#), a!(TrustType::InstalledSoftware, 9, 0.7)),
//         (s_add!(HOME.as_str(), r#"\Documents\Tencent Files"#), a!(TrustType::InstalledSoftware, 8, 0.7)),
//         (s_add!(LOCALAPPDATA.as_str(), r#"\Feishu"#), a!(TrustType::InstalledSoftware, 8, 0.8)),
//         (s_add!(APPDATA.as_str(), r#"\Telegram Desktop"#), a!(TrustType::InstalledSoftware, 8, 0.9)),
//         (s_add!(APPDATA.as_str(), r#"\discord"#), a!(TrustType::InstalledSoftware, 8, 0.9)),
//     ];
//
//     // Linux 专属对抗与痕迹目录
//     let dirs_lin: Vec<(String, ScoreAction)> = vec![
//         (sss!("/opt/cuckoo"), a!(SandboxType::Cuckoo, 10, 0.9)),
//         (sss!("/opt/cape"), a!(SandboxType::CAPE, 10, 0.9)),
//         (sss!("/home/sandbox"), a!(SandboxType::Unknown, 9, 0.9)),
//         (sss!("/home/malware"), a!(SoftwareType::Analysis, 6, 0.5)),
//         (sss!("/var/lib/kubelet"), a!(ContainerType::Kubernetes, 8, 0.8)),
//         (s_add!(HOME.as_str(), "/.steam"), a!(TrustType::Game, 10, 0.95)),
//         (s_add!(HOME.as_str(), "/.minecraft"), a!(TrustType::Game, 8, 0.85)),
//         (s_add!(HOME.as_str(), "/.mozilla/firefox"), a!(TrustType::UserTraces, 7, 0.90)),
//     ];
//
//     // macOS 专属痕迹目录
//     let dirs_mac: Vec<(String, ScoreAction)> = vec![
//         (s_add!(HOME.as_str(), "/Library/Application Support/Steam"), a!(TrustType::Game, 10, 0.95)),
//     ];
//
//
//     let mut target_dirs = dirs;
//
//     #[cfg(target_os = "windows")]
//     target_dirs.extend(dirs_win);
//
//     #[cfg(target_os = "linux")]
//     target_dirs.extend(dirs_lin);
//
//     #[cfg(target_os = "macos")]
//     target_dirs.extend(dirs_mac);
//
//     for (path, action) in target_dirs {
//         chk_dir(&mut actions, &path, action).await;
//     }
//
//     if !actions.is_empty() {
//         env.lock().await.add_all(actions);
//     }
// }
//
// async fn chk_file_size_min(actions: &mut Vec<ScoreAction>, path: &str, min_size: u64, mut action: ScoreAction) {
//     if let Ok(meta) = tokio_fs::symlink_metadata(path).await {
//         if meta.is_file() {
//             let file_size = meta.len();
//             if file_size >= min_size {
//                 action.set_msg(s_add!("File found[" , format_args!("{: >6.1}", file_size as f64 / 1024.0),"KB]: ", path));
//                 actions.push(action);
//             }
//         }
//     }
// }
//
// pub async fn check_files(env: Arc<Mutex<Environment>>) {
//     macro_rules! a {
//         ($action_type:expr, $score:expr, $confidence:expr) => {{
//             action!($action_type, $crate::sandbox::ScoreType::File, $score, $confidence)
//         }};
//     }
//     let mut actions = Vec::<ScoreAction>::new();
//
//     let drive = SYS_DRIVE.as_str();
//     let pf = PROG_FILES.as_str();
//     let pf86 = PROG_FILES_X86.as_str();
//     let home = HOME.as_str();
//     let localappdata = LOCALAPPDATA.as_str();
//     let appdata = APPDATA.as_str();
//     let system32 = SYSTEM32.as_str();
//
//     // ==========================================
//     // 1. 通用跨平台文件 (不区分 OS 挂载)
//     // ==========================================
//     let files: Vec<(String, u64, ScoreAction)> = vec![
//         // 开发痕迹与敏感配置
//         (s_add!(home, "/.ssh/id_rsa"), 256, a!(TrustType::Development, 8, 0.95)),
//         (s_add!(home, "/.ssh/id_ed25519"), 64, a!(TrustType::Development, 8, 0.95)),
//         (s_add!(home, "/.gitconfig"), 10, a!(TrustType::Development, 7, 0.85)),
//         (s_add!(home, "/.aws/credentials"), 32, a!(TrustType::Development, 7, 0.95)),
//         (s_add!(home, "/.kube/config"), 64, a!(TrustType::Development, 7, 0.95)),
//
//         // 终端操作历史
//         (s_add!(home, "/.bash_history"), 512, a!(TrustType::PersonalFiles, 3, 0.60)),
//         (s_add!(home, "/.zsh_history"), 512, a!(TrustType::PersonalFiles, 3, 0.60)),
//         (s_add!(home, "/.psql_history"), 512, a!(TrustType::PersonalFiles, 3, 0.65)),
//     ];
//
//     // ==========================================
//     // 2. Windows 专属对抗与可信痕迹
//     // ==========================================
//     let files_win: Vec<(String, u64, ScoreAction)> = vec![
//         // === 沙箱代理与专属 DLL ===
//         (s_add!(system32, r"\cuckoomon.dll"), 256, a!(SandboxType::Cuckoo, 10, 0.9)),
//         (s_add!(drive, r"\cuckoomon.dll"), 256, a!(SandboxType::Cuckoo, 10, 0.9)),
//         (s_add!(drive, r"\capemon.dll"), 256, a!(SandboxType::CAPE, 10, 0.9)),
//         (s_add!(drive, r"\agent.exe"), 1024, a!(SandboxType::Cuckoo, 10, 0.9)),
//         (s_add!(drive, r"\analysis.log"), 0, a!(SandboxType::Unknown, 8, 0.8)),
//         (s_add!(drive, r"\sandbox.log"), 0, a!(SandboxType::Unknown, 8, 0.8)),
//
//         // === 常见虚拟机内核驱动 ===
//         (s_add!(drive, r"\Windows\System32\drivers\vmhgfs.sys"), 256, a!(VirtualMachineType::VMware, 9, 0.8)),
//         (s_add!(drive, r"\Windows\System32\drivers\vmmouse.sys"), 256, a!(VirtualMachineType::VMware, 9, 0.8)),
//         (s_add!(drive, r"\Windows\System32\drivers\vmci.sys"), 256, a!(VirtualMachineType::VMware, 9, 0.8)),
//         (s_add!(drive, r"\Windows\System32\drivers\balloon.sys"), 256, a!(VirtualMachineType::VMware, 8, 0.7)),
//         (s_add!(drive, r"\Windows\System32\drivers\prl_tg.sys"), 256, a!(VirtualMachineType::Parallels, 8, 0.8)),
//         (s_add!(drive, r"\Windows\System32\drivers\viocrypt.sys"), 256, a!(VirtualMachineType::KVM, 9, 0.8)),
//         (s_add!(drive, r"\Windows\System32\drivers\vioscsi.sys"), 256, a!(VirtualMachineType::KVM, 9, 0.8)),
//         (s_add!(drive, r"\Windows\System32\drivers\vmbus.sys"), 256, a!(VirtualMachineType::HyperV, 9, 0.8)),
//         (s_add!(drive, r"\Windows\System32\drivers\hypervideo.sys"), 256, a!(VirtualMachineType::HyperV, 9, 0.8)),
//
//         // === 逆向与分析工具主程序 ===
//         (s_add!(pf, r"\Sysinternals\ProcMon.exe"), 1024, a!(SoftwareType::Analysis, 5, 0.4)),
//         (s_add!(pf, r"\Process Hacker 2\ProcessHacker.exe"), 1024, a!(SoftwareType::Analysis, 6, 0.5)),
//         (s_add!(drive, r"\x64dbg\release\x64dbg.exe"), 1024, a!(SoftwareType::Debugger, 8, 0.7)),
//         (s_add!(drive, r"\x64dbg\release\x32dbg.exe"), 1024, a!(SoftwareType::Debugger, 8, 0.7)),
//         (s_add!(drive, r"\Ghidra\ghidraRun.bat"), 64, a!(SoftwareType::Debugger, 7, 0.6)),
//         (s_add!(pf86, r"\IDA\ida64.exe"), 1024, a!(SoftwareType::Debugger, 7, 0.6)),
//         (s_add!(drive, r"\Windows\System32\drivers\npcap.sys"), 256, a!(SoftwareType::Analysis, 6, 0.6)),
//
//
//         // === Wine 专属可执行文件与驱动 ===
//         (s_add!(system32, r"\winecfg.exe"), 256, a!(EmulatorType::Wine, 10, 0.95)),
//         (s_add!(system32, r"\wineboot.exe"), 256, a!(EmulatorType::Wine, 10, 0.95)),
//         (s_add!(system32, r"\winedbg.exe"), 256, a!(EmulatorType::Wine, 10, 0.95)),
//         (s_add!(system32, r"\winemine.exe"), 256, a!(EmulatorType::Wine, 8, 0.90)), // Wine 自带的扫雷
//         (s_add!(system32, r"\winex11.drv"), 256, a!(EmulatorType::Wine, 9, 0.95)), // Linux/X11 下的 Wine 驱动
//         (s_add!(system32, r"\winemac.drv"), 256, a!(EmulatorType::Wine, 9, 0.90)), // macOS 下的 Wine 驱动
//         // === Wine Z盘映射 (Linux宿主机文件暴露) ===
//         (sss!(r"Z:\etc\passwd"), 256, a!(EmulatorType::Wine, 10, 0.95)),
//         (sss!(r"Z:\etc\fstab"), 256, a!(EmulatorType::Wine, 10, 0.95)),
//         (sss!(r"Z:\bin\bash"), 256, a!(EmulatorType::Wine, 10, 0.95)),
//         (sss!(r"Z:\bin\sh"), 256, a!(EmulatorType::Wine, 10, 0.95)),
//         (sss!(r"Z:\bin\ls"), 256, a!(EmulatorType::Wine, 10, 0.95)),
//         (sss!(r"Z:\bin\cd"), 256, a!(EmulatorType::Wine, 10, 0.95)),
//         // === Wine 注册表映射文件 ===
//         (s_add!(drive, r"\Windows\system.reg"), 0, a!(SandboxType::Unknown, 10, 0.95)),
//         (s_add!(drive, r"\Windows\user.reg"), 0, a!(SandboxType::Unknown, 10, 0.95)),
//         (s_add!(drive, r"\Windows\userdef.reg"), 0, a!(SandboxType::Unknown, 10, 0.95)),
//
//         // === 用户高信誉度状态库 ===
//         (s_add!(localappdata, "/Microsoft/Edge/User Data/Default/History"), 1024, a!(TrustType::UserTraces, 8, 0.85)),
//         (s_add!(home, "/ntuser.dat"), 1024, a!(TrustType::UserAccounts, 5, 0.60)),
//         (s_add!(appdata, "/Telegram Desktop/tdata/key_datas"), 64, a!(TrustType::InstalledSoftware, 9, 0.95)),
//     ];
//
//     // ==========================================
//     // 3. Linux/容器 专属指纹
//     // ==========================================
//     let files_lin: Vec<(String, u64, ScoreAction)> = vec![
//         (sss!("/.dockerenv"), 0, a!(ContainerType::Docker, 7, 0.8)),
//         (sss!("/run/.containerenv"), 0, a!(ContainerType::Podman, 7, 0.8)),
//         (sss!("/run/docker.sock"), 0, a!(ContainerType::Docker, 8, 0.9)),
//         (sss!("/usr/sbin/VBoxService"), 512, a!(VirtualMachineType::VirtualBox, 8, 0.8)),
//         (sss!("/dev/virtio-ports"), 0, a!(VirtualMachineType::KVM, 8, 0.8)),
//         (sss!("/var/run/containerd/containerd.sock"), 0, a!(ContainerType::Containerd, 8, 0.8)),
//         (sss!("/var/run/crio/crio.sock"), 0, a!(ContainerType::Unknown, 7, 0.7)),
//         (sss!("/var/run/secrets/kubernetes.io"), 0, a!(ContainerType::Kubernetes, 9, 0.9)),
//         (sss!("/proc/sys/fs/binfmt_misc/WSLInterop"), 0, a!(ContainerType::Wsl, 9, 0.9)),
//     ];
//
//     // ==========================================
//     // 4. macOS 专属指纹
//     // ==========================================
//     let files_mac: Vec<(String, u64, ScoreAction)> = vec![
//         (sss!("/Library/Frameworks/ParallelsDesktop.framework"), 0, a!(VirtualMachineType::Parallels, 9, 0.9)),
//     ];
//
//     let mut target_files = files;
//
//     #[cfg(target_os = "windows")]
//     target_files.extend(files_win);
//
//     #[cfg(target_os = "linux")]
//     target_files.extend(files_lin);
//
//     #[cfg(target_os = "macos")]
//     target_files.extend(files_mac);
//
//     for (path, min_size, action) in target_files {
//         chk_file_size_min(&mut actions, &path, min_size, action).await;
//     }
//
//     if !actions.is_empty() {
//         env.lock().await.add_all(actions);
//     }
// }
//
// /// 已过时和被审计的对象
// // chk_file!(format!(r"{}\agent.pyw", drive),                      sandbox, SandboxType::Unknown, 9, 0.8);
// // chk_file!(format!(r"{}\analyzer.py", drive), sandbox, SandboxType::Unknown, 9, 0.8);
// // chk_file!(sss!("/opt/cuckoo/agent.py"),                    sandbox, SandboxType::Cuckoo,    10, 0.9);
// // chk_file!(format!(r"{}\Windows\System32\drivers\VBoxGuest.sys", drive), virtual_machine, VirtualMachineType::VirtualBox, 9, 0.8);
// // chk_file!(format!(r"{}\Windows\System32\drivers\VBoxMouse.sys", drive), virtual_machine, VirtualMachineType::VirtualBox, 9, 0.8);
//
// async fn chk_files_too_few(
//     actions: &mut Vec<ScoreAction>,
//     dir: &str,
//     mut thresholds: Vec<(u32, ScoreAction)>,
// ) {
//     if thresholds.is_empty() { return; }
//
//     thresholds.sort_by_key(|(min, _)| *min);
//
//     let max_min = thresholds.last().unwrap().0;
//
//     if let Ok(mut entries) = tokio_fs::read_dir(dir).await {
//         let mut count: u32 = 0;
//
//         while let Ok(Some(_)) = entries.next_entry().await {
//             count += 1;
//             if count >= max_min { break; }
//         }
//
//
//         for (min, mut action) in thresholds {
//             if count < min {
//                 action.set_msg(s_add!("Too few files[", count, "]: ", dir));
//                 actions.push(action);
//                 break;
//             }
//         }
//     }
// }
//
//
// async fn chk_files_sufficient(
//     actions: &mut Vec<ScoreAction>,
//     dir: &str,
//     mut thresholds: Vec<(u32, ScoreAction)>,
// ) {
//     if thresholds.is_empty() { return; }
//
//     thresholds.sort_by(|a, b| b.0.cmp(&a.0));
//
//     let max_threshold = thresholds.first().unwrap().0;
//
//     if let Ok(mut entries) = tokio_fs::read_dir(dir).await {
//         let mut count: u32 = 0;
//
//         while let Ok(Some(_)) = entries.next_entry().await {
//             count += 1;
//             if count >= max_threshold { break; }
//         }
//
//         for (min, mut action) in thresholds {
//             if count >= min {
//                 action.set_msg(s_add!("Enough files[", count, "+]: ", dir));
//                 actions.push(action);
//                 break;
//             }
//         }
//     }
// }
//
// pub async fn check_file_totals(env: Arc<Mutex<Environment>>) {
//     macro_rules! a {
//         ($action_type:expr, $score:expr, $confidence:expr) => {{
//             action!($action_type, $crate::sandbox::ScoreType::UserActivity, $score, $confidence)
//         }};
//     }
//     let mut actions: Vec<ScoreAction> = Vec::new();
//
//     let home = HOME.as_str();
//     let appdata = APPDATA.as_str();
//     let localappdata = LOCALAPPDATA.as_str();
//     let drive = SYS_DRIVE.as_str();
//
//     // ==========================================
//     // 1. 数量过少检测 (dirs_too_few) -> 沙箱特征
//     // ==========================================
//     let dirs_too_few: Vec<(String, Vec<(u32, ScoreAction)>)> = vec![
//         (s_add!(home), vec![(20, a!(SandboxType::Unknown, 5, 0.65)), (10, a!(SandboxType::Unknown, 7, 0.65)), (5, a!(SandboxType::Unknown, 9, 0.685))]),
//     ];
//
//     let dirs_too_few_win: Vec<(String, Vec<(u32, ScoreAction)>)> = vec![
//         (s_add!(appdata, r"\Microsoft\Windows\Recent"), vec![(20, a!(SandboxType::Unknown, 4, 0.60))]),
//         (s_add!(localappdata, r"\Temp"), vec![(300, a!(SandboxType::Unknown, 4, 0.60))]),
//         (s_add!(drive, r"\Windows\Temp"), vec![(100, a!(SandboxType::Unknown, 4, 0.60))]),
//         (s_add!(drive, r"\Windows\Prefetch"), vec![(50, a!(SandboxType::Unknown, 5, 0.70))]),
//         (s_add!(drive, r"\Windows\Fonts"), vec![(330, a!(SandboxType::Unknown, 4, 0.65))]),
//         (s_add!(PROG_FILES.as_str()), vec![(20, a!(SandboxType::Unknown, 4, 0.60))]),
//         (s_add!(PROG_FILES_X86.as_str()), vec![(18, a!(SandboxType::Unknown, 4, 0.60))]),
//         (s_add!(appdata, r"\Microsoft\Windows\Start Menu\Programs"), vec![(30, a!(SandboxType::Unknown, 4, 0.60))]),
//         (s_add!(drive, r"\$Recycle.Bin"), vec![(5, a!(SandboxType::Unknown, 5, 0.70))]),
//     ];
//
//     let dirs_too_few_lin: Vec<(String, Vec<(u32, ScoreAction)>)> = vec![
//         (sss!("/tmp"), vec![(20, a!(SandboxType::Unknown, 4, 0.50)),(100, a!(SandboxType::Unknown, 4, 0.20))]),
//         (sss!("/var/log"), vec![(15, a!(SandboxType::Unknown, 4, 0.60))]),
//         (sss!("/dev"), vec![(20, a!(SandboxType::Unknown, 4, 0.60)),(80, a!(SandboxType::Unknown, 4, 0.30))]),
//         (sss!("/usr/bin"), vec![(1000, a!(SandboxType::Unknown, 1, 0.20)),(500, a!(SandboxType::Unknown, 5, 0.70)),(300, a!(SandboxType::Unknown, 5, 0.90))]),
//     ];
//
//     let dirs_too_few_mac: Vec<(String, Vec<(u32, ScoreAction)>)> = vec![
//         (sss!("/Applications"), vec![(10, a!(SandboxType::Unknown, 4, 0.60))]),
//     ];
//
//     // ==========================================
//     // 2. 数量充足检测 (dirs_sufficient) -> 真实环境可信特征
//     // ==========================================
//     let dirs_sufficient: Vec<(String, Vec<(u32, ScoreAction)>)> = vec![
//         (s_add!(home, "/.gradle/caches/modules-2/files-2.1"), vec![
//             (100, a!(TrustType::Development, 7, 0.80)),
//             (200, a!(TrustType::Development, 8, 0.90)),
//         ]),
//         (s_add!(home, "/.vscode/extensions"), vec![
//             (5, a!(TrustType::Development, 6, 0.75)),
//             (15, a!(TrustType::Development, 7, 0.85)),
//             (30, a!(TrustType::Development, 8, 0.90)),
//         ]),
//         (s_add!(home, "/Documents/WeChat Files"), vec![
//             (10, a!(TrustType::InstalledSoftware, 7, 0.80)),
//             (50, a!(TrustType::InstalledSoftware, 8, 0.90)),
//             (200, a!(TrustType::InstalledSoftware, 9, 0.95)),
//         ]),
//         (s_add!(home, "/Documents/Tencent Files"), vec![
//             (10, a!(TrustType::InstalledSoftware, 7, 0.80)),
//             (50, a!(TrustType::InstalledSoftware, 8, 0.90)),
//         ]),
//         (s_add!(home, "/Documents"), vec![
//             (20, a!(TrustType::PersonalFiles, 5, 0.60)),
//             (50, a!(TrustType::PersonalFiles, 6, 0.70)),
//             (100, a!(TrustType::PersonalFiles, 7, 0.80)),
//         ]),
//         (s_add!(home, "/Downloads"), vec![
//             (30, a!(TrustType::PersonalFiles, 5, 0.60)),
//             (100, a!(TrustType::PersonalFiles, 6, 0.70)),
//             (300, a!(TrustType::PersonalFiles, 7, 0.80)),
//         ]),
//         (s_add!(home, "/Pictures"), vec![
//             (50, a!(TrustType::PersonalFiles, 6, 0.70)),
//             (200, a!(TrustType::PersonalFiles, 7, 0.80)),
//             (1000, a!(TrustType::PersonalFiles, 8, 0.90)),
//         ]),
//         (s_add!(home, "/Desktop"), vec![
//             (10, a!(TrustType::PersonalFiles, 5, 0.60)),
//             (30, a!(TrustType::PersonalFiles, 6, 0.70)),
//         ]),
//         (s_add!(home, "/OneDrive"), vec![
//             (20, a!(TrustType::PersonalFiles, 7, 0.80)),
//             (100, a!(TrustType::PersonalFiles, 8, 0.90)),
//             (500, a!(TrustType::PersonalFiles, 9, 0.95)),
//         ]),
//         (s_add!(home, "/Dropbox"), vec![
//             (20, a!(TrustType::PersonalFiles, 7, 0.80)),
//             (100, a!(TrustType::PersonalFiles, 8, 0.90)),
//         ]),
//     ];
//
//     let dirs_sufficient_win: Vec<(String, Vec<(u32, ScoreAction)>)> = vec![
//         (s_add!(PROG_FILES_X86.as_str(), r"\Steam\steamapps\common"), vec![
//             (3, a!(TrustType::Game, 7, 0.75)),
//             (10, a!(TrustType::Game, 8, 0.85)),
//             (30, a!(TrustType::Game, 9, 0.92)),
//             (100, a!(TrustType::Game, 10, 0.98)),
//         ]),
//         (s_add!(PROG_FILES.as_str(), r"\Epic Games"), vec![
//             (2, a!(TrustType::Game, 7, 0.75)),
//             (5, a!(TrustType::Game, 8, 0.85)),
//             (10, a!(TrustType::Game, 9, 0.90)),
//         ]),
//         (sss!(r"D:\WeGamePlatform"), vec![
//             (3, a!(TrustType::Game, 7, 0.75)),
//             (10, a!(TrustType::Game, 8, 0.85)),
//         ]),
//         (s_add!(WINDIR.as_str(), r"\System32\winevt\Logs"), vec![
//             (100, a!(TrustType::UserTraces, 6, 0.80)),
//             (200, a!(TrustType::UserTraces, 7, 0.90)),
//         ]),
//         (s_add!(PROGRAMDATA.as_str(), r"\Microsoft\Wlansvc\Profiles\Interfaces"), vec![
//             (3, a!(TrustType::Network, 6, 0.70)),
//             (10, a!(TrustType::Network, 7, 0.85)),
//             (20, a!(TrustType::Network, 8, 0.92)),
//         ]),
//         (s_add!(localappdata, r"\Microsoft\Windows"), vec![
//             (50, a!(TrustType::UserTraces, 5, 0.60)),
//             (100, a!(TrustType::UserTraces, 6, 0.70)),
//             (200, a!(TrustType::UserTraces, 7, 0.80)),
//         ]),
//         (s_add!(drive, r"\Windows\Fonts"), vec![
//             (200, a!(TrustType::InstalledSoftware, 6, 0.75)),
//         ]),
//         (s_add!(PROGRAMDATA.as_str(), r"\Microsoft\Windows\Start Menu\Programs"), vec![
//             (20, a!(TrustType::InstalledSoftware, 6, 0.70)),
//             (50, a!(TrustType::InstalledSoftware, 7, 0.80)),
//             (100, a!(TrustType::InstalledSoftware, 8, 0.90)),
//         ]),
//         (s_add!(appdata, r"\Microsoft\Windows\Start Menu\Programs"), vec![
//             (10, a!(TrustType::InstalledSoftware, 5, 0.65)),
//             (30, a!(TrustType::InstalledSoftware, 6, 0.75)),
//         ]),
//     ];
//
//     let dirs_sufficient_lin: Vec<(String, Vec<(u32, ScoreAction)>)> = vec![
//         (s_add!(home, "/.steam/steam/steamapps/common"), vec![
//             (3, a!(TrustType::Game, 7, 0.7)),
//             (10, a!(TrustType::Game, 8, 0.85)),
//             (30, a!(TrustType::Game, 9, 0.92)),
//             (100, a!(TrustType::Game, 10, 0.98)),
//         ]),
//         (sss!("/var/log"), vec![
//             (50, a!(TrustType::UserTraces, 6, 0.80)),
//             (100, a!(TrustType::UserTraces, 7, 0.90)),
//         ]),
//         (sss!("/sys/bus/usb/devices"), vec![
//             (11, action!(TrustType::PhysicalDevices, ScoreType::UserActivity, 8, 0.8)),
//         ]),
//     ];
//
//     let dirs_sufficient_mac: Vec<(String, Vec<(u32, ScoreAction)>)> = vec![];
//
//
//     let mut target_too_few = dirs_too_few;
//     #[cfg(target_os = "windows")]
//     target_too_few.extend(dirs_too_few_win);
//     #[cfg(target_os = "linux")]
//     target_too_few.extend(dirs_too_few_lin);
//     #[cfg(target_os = "macos")]
//     target_too_few.extend(dirs_too_few_mac);
//
//     for (dir, thresholds) in target_too_few {
//         chk_files_too_few(&mut actions, &dir, thresholds).await;
//     }
//
//     let mut target_sufficient = dirs_sufficient;
//     #[cfg(target_os = "windows")]
//     target_sufficient.extend(dirs_sufficient_win);
//     #[cfg(target_os = "linux")]
//     target_sufficient.extend(dirs_sufficient_lin);
//     #[cfg(target_os = "macos")]
//     target_sufficient.extend(dirs_sufficient_mac);
//
//     for (dir, thresholds) in target_sufficient {
//         chk_files_sufficient(&mut actions, &dir, thresholds).await;
//     }
//
//     if !actions.is_empty() {
//         env.lock().await.add_all(actions);
//     }
// }
//
// async fn chk_file_content(
//     actions: &mut Vec<ScoreAction>,
//     file: &str,
//     keyword: &str,
//     max_bytes: u64,
//     mut action: ScoreAction,
// ) {
//     let Ok(f) = tokio_fs::File::open(file).await else { return; };
//     let mut reader = BufReader::new(f);
//     let mut line = Vec::with_capacity(512);
//     let mut read = 0u64;
//     let needle = keyword.to_lowercase();
//
//     loop {
//         if max_bytes > 0 && read >= max_bytes {
//             break;
//         }
//
//         line.clear();
//         match reader.read_until(b'\n', &mut line).await {
//             Ok(0) | Err(_) => break,
//             Ok(n) => read += n as u64,
//         }
//
//         let mut raw_str = String::from_utf8_lossy(&line).into_owned();
//         raw_str.retain(|c| c != '\0');
//
//         if raw_str.to_lowercase().contains(&needle) {
//             action.set_msg(s_add!(file, " contains '", keyword, "'"));
//             actions.push(action);
//             return; // 找到即返回，无需继续读取
//         }
//     }
// }
// pub async fn check_file_contents(env: Arc<Mutex<Environment>>) {
//     macro_rules! a {
//         ($action_type:expr, $score_type:ident, $score:expr, $confidence:expr) => {{
//             action!($action_type, $crate::sandbox::ScoreType::$score_type, $score, $confidence)
//         }};
//     }
//
//     let mut actions: Vec<ScoreAction> = Vec::new();
//     let home = HOME.as_str();
//     let sys_drive = SYS_DRIVE.as_str();
//     let appdata = APPDATA.as_str();
//
//     // ==========================================
//     // 1. 跨平台 / 通用环境可信度痕迹 (items_generic)
//     // ==========================================
//     let items_generic: Vec<(String, String, u64, ScoreAction)> = vec![
//         (s_add!(home, "/.gitconfig"), sss!("email = "), 16 * KB, a!(TrustType::Development, FileContent, 9, 0.9)),
//         (s_add!(home, "/.gitconfig"), sss!("name = "), 16 * KB, a!(TrustType::Development, FileContent, 9, 0.9)),
//         (s_add!(home, "/.ssh/config"), sss!("HostName "), 32 * KB, a!(TrustType::Development, FileContent, 8, 0.85)),
//         (s_add!(home, "/.ssh/config"), sss!("IdentityFile "), 32 * KB, a!(TrustType::Development, FileContent, 9, 0.9)),
//         (s_add!(home, "/.ssh/known_hosts"), sss!("ssh-rsa "), 128 * KB, a!(TrustType::Network, FileContent, 7, 0.8)),
//         (s_add!(home, "/.ssh/known_hosts"), sss!("ecdsa-sha2-"), 128 * KB, a!(TrustType::Network, FileContent, 7, 0.8)),
//         (s_add!(home, "/.aws/credentials"), sss!("aws_access_key_id"), 16 * KB, a!(TrustType::Development, FileContent, 10, 0.95)),
//         (s_add!(home, "/.npmrc"), sss!("registry="), 16 * KB, a!(TrustType::Development, FileContent, 7, 0.8)),
//     ];
//
//     // ==========================================
//     // 2. Windows 专属特征与控制台历史 (items_win)
//     // ==========================================
//     let items_win: Vec<(String, String, u64, ScoreAction)> = vec![
//         (s_add!(sys_drive, r"\Windows\inf\setupapi.dev.log"), sss!("vboxguest"), 5120 * KB, a!(VirtualMachineType::VirtualBox, Driver, 9, 0.8)),
//         (s_add!(sys_drive, r"\Windows\inf\setupapi.dev.log"), sss!("vboxvideo"), 5120 * KB, a!(VirtualMachineType::VirtualBox, Driver, 9, 0.8)),
//         (s_add!(sys_drive, r"\Windows\inf\setupapi.dev.log"), sss!("vmware"), 5120 * KB, a!(VirtualMachineType::VMware, Driver, 9, 0.8)),
//         (s_add!(sys_drive, r"\Windows\inf\setupapi.dev.log"), sss!("ven_80ee"), 5120 * KB, a!(VirtualMachineType::VirtualBox, Driver, 9, 0.85)),
//         (s_add!(appdata, r"\Microsoft\Windows\PowerShell\PSReadLine\ConsoleHost_history.txt"), sss!("git "), 256 * KB, a!(TrustType::Development, UserActivity, 8, 0.85)),
//         (s_add!(appdata, r"\Microsoft\Windows\PowerShell\PSReadLine\ConsoleHost_history.txt"), sss!("npm "), 256 * KB, a!(TrustType::Development, UserActivity, 8, 0.85)),
//         (s_add!(appdata, r"\Microsoft\Windows\PowerShell\PSReadLine\ConsoleHost_history.txt"), sss!("ping "), 256 * KB, a!(TrustType::Network, UserActivity, 5, 0.6)),
//         (s_add!(appdata, r"\Microsoft\Windows\PowerShell\PSReadLine\ConsoleHost_history.txt"), sss!("ssh "), 256 * KB, a!(TrustType::Network, UserActivity, 8, 0.85)),
//     ];
//
//     // ==========================================
//     // 3. Linux 虚拟化指纹与 Shell 历史 (items_lin)
//     // ==========================================
//     let items_lin: Vec<(String, String, u64, ScoreAction)> = vec![
//         (sss!("/proc/cpuinfo"), sss!("hypervisor"), 64 * KB, a!(VirtualMachineType::Unknown, Cpu, 8, 0.8)),
//         (sss!("/proc/cpuinfo"), sss!("KVMKVMKVM"), 64 * KB, a!(VirtualMachineType::KVM, Cpu, 10, 0.9)),
//         (sss!("/proc/cpuinfo"), sss!("VMwareVMware"), 64 * KB, a!(VirtualMachineType::VMware, Cpu, 10, 0.9)),
//         (sss!("/proc/cpuinfo"), sss!("XenVMMXenVMM"), 64 * KB, a!(VirtualMachineType::Xen, Cpu, 10, 0.9)),
//         (sss!("/proc/cpuinfo"), sss!("Microsoft Hv"), 64 * KB, a!(VirtualMachineType::HyperV, Cpu, 9, 0.8)),
//         (sss!("/proc/version"), sss!("cuckoo"), 4 * KB, a!(SandboxType::Cuckoo, FileContent, 10, 0.9)),
//         (sss!("/proc/version"), sss!("sandbox"), 4 * KB, a!(SandboxType::Unknown, FileContent, 8, 0.8)),
//         (sss!("/proc/1/cgroup"), sss!("docker"), 16 * KB, a!(ContainerType::Docker, FileContent, 8, 0.8)),
//         (sss!("/proc/1/cgroup"), sss!("kubepods"), 16 * KB, a!(ContainerType::Kubernetes, FileContent, 7, 0.7)),
//         (sss!("/proc/1/cgroup"), sss!("lxc"), 16 * KB, a!(ContainerType::LXC, FileContent, 8, 0.8)),
//         (sss!("/proc/scsi/scsi"), sss!("VBOX"), 32 * KB, a!(VirtualMachineType::VirtualBox, FileContent, 9, 0.8)),
//         (sss!("/proc/scsi/scsi"), sss!("VMWARE"), 32 * KB, a!(VirtualMachineType::VMware, FileContent, 9, 0.8)),
//         (sss!("/proc/scsi/scsi"), sss!("QEMU"), 32 * KB, a!(EmulatorType::QemuTCG, FileContent, 9, 0.8)),
//         (sss!("/proc/mounts"), sss!("vboxsf"), 64 * KB, a!(VirtualMachineType::VirtualBox, FileContent, 9, 0.8)),
//         (sss!("/proc/mounts"), sss!("vmhgfs"), 64 * KB, a!(VirtualMachineType::VMware, FileContent, 9, 0.8)),
//         (sss!("/proc/mounts"), sss!("overlay / "), 128*KB, a!(ContainerType::Docker, FileContent, 9, 0.90)),
//         (sss!("/proc/mounts"), sss!("/var/lib/docker"), 128*KB, a!(ContainerType::Docker, FileContent, 8, 0.40)),
//         (sss!("/proc/mounts"), sss!("/run/docker.sock"), 128*KB, a!(ContainerType::Docker, FileContent, 10, 0.90)),
//         (sss!("/proc/mounts"), sss!("/dev/mapper/nvme"), 128*KB, a!(TrustType::PhysicalDevices, FileContent, 8, 0.80)),
//         (sss!("/proc/mounts"), sss!("/@home"), 128*KB, a!(TrustType::PhysicalDevices, FileContent, 5, 0.40)),
//         (sss!("/proc/mounts"), sss!("/@swap"), 128*KB, a!(TrustType::PhysicalDevices, FileContent, 5, 0.40)),
//         (sss!("/proc/mounts"), sss!("/.snapshots"), 128*KB, a!(TrustType::PhysicalDevices, FileContent, 5, 0.40)),
//         (sss!("/proc/net/dev"), sss!("vboxnet"), 16 * KB, a!(VirtualMachineType::VirtualBox, Network, 8, 0.8)),
//         (sss!("/proc/net/dev"), sss!("vmnet"), 16 * KB, a!(VirtualMachineType::VMware, Network, 8, 0.8)),
//         (sss!("/sys/class/dmi/id/product_name"), sss!("VirtualBox"), KB, a!(VirtualMachineType::VirtualBox, Dmi, 10, 0.9)),
//         (sss!("/sys/class/dmi/id/product_name"), sss!("VMware"), KB, a!(VirtualMachineType::VMware, Dmi, 10, 0.9)),
//         (sss!("/sys/class/dmi/id/product_name"), sss!("QEMU"), KB, a!(EmulatorType::QemuTCG, Dmi, 10, 0.9)),
//         (sss!("/sys/class/dmi/id/sys_vendor"), sss!("innotek GmbH"), KB, a!(VirtualMachineType::VirtualBox, Dmi, 10, 0.9)),
//         (sss!("/sys/class/dmi/id/sys_vendor"), sss!("VMware, Inc."), KB, a!(VirtualMachineType::VMware, Dmi, 10, 0.9)),
//         (sss!("/sys/class/dmi/id/bios_vendor"), sss!("SeaBIOS"), KB, a!(EmulatorType::Unknown, Bios, 7, 0.7)),
//         (sss!("/var/log/dmesg"), sss!("Hypervisor detected"), 512 * KB, a!(VirtualMachineType::Unknown, Driver, 8, 0.8)),
//         (sss!("/var/log/dmesg"), sss!("VMware vmxnet"), 512 * KB, a!(VirtualMachineType::VMware, Driver, 9, 0.8)),
//         (sss!("/proc/version"), sss!("Microsoft"), 4 * KB, a!(ContainerType::Wsl, FileContent, 8, 0.8)),
//         (sss!("/proc/cpuinfo"), sss!("AuthenticAMD"), 64 * KB, a!(TrustType::PhysicalDevices, Cpu, 6, 0.3)),
//         (sss!("/proc/cpuinfo"), sss!("GenuineIntel"), 64 * KB, a!(TrustType::PhysicalDevices, Cpu, 6, 0.3)),
//         (s_add!(home, "/.bash_history").into_string(), sss!("git commit"), 256 * KB, a!(TrustType::Development, UserActivity, 8, 0.85)),
//         (s_add!(home, "/.bash_history").into_string(), sss!("ssh "), 256 * KB, a!(TrustType::Network, UserActivity, 7, 0.85)),
//         (s_add!(home, "/.bash_history").into_string(), sss!("curl "), 256 * KB, a!(TrustType::Network, UserActivity, 7, 0.85)),
//         (s_add!(home, "/.bash_history").into_string(), sss!("docker run"), 256 * KB, a!(TrustType::Development, UserActivity, 7, 0.85)),
//         (sss!("/sys/devices/virtual/dmi/id/product_name"), sss!("QEMU"), KB, a!(EmulatorType::QemuTCG, Driver, 9, 0.8)),
//     ];
//
//     // ==========================================
//     // 4. macOS 专属痕迹检测项 (items_mac)
//     // ==========================================
//     let items_mac: Vec<(String, String, u64, ScoreAction)> = vec![
//         (s_add!(home, "/.bash_history").into_string(), sss!("git commit"), 256 * KB, a!(TrustType::Development, UserActivity, 8, 0.85)),
//         (s_add!(home, "/.bash_history").into_string(), sss!("ssh "), 256 * KB, a!(TrustType::Network, UserActivity, 7, 0.85)),
//         (s_add!(home, "/.bash_history").into_string(), sss!("curl "), 256 * KB, a!(TrustType::Network, UserActivity, 7, 0.85)),
//         (s_add!(home, "/.bash_history").into_string(), sss!("docker run"), 256 * KB, a!(TrustType::Development, UserActivity, 7, 0.85)),
//     ];
//
//     #[cfg(unix)]
//     {
//         if let Ok(hostname) = tokio_fs::read_to_string("/proc/sys/kernel/hostname").await {
//             let h = hostname.trim();
//             if h.len() == 12 && h.chars().all(|c| c.is_ascii_hexdigit()) {
//                 actions.push(action!(ContainerType::Docker,ScoreType::FileContent,s_add!("Detected Docker hostname: ",h),8,0.8))
//             }
//         }
//     }
//
//
//     let mut target_items = items_generic;
//     #[cfg(target_os = "windows")]
//     target_items.extend(items_win);
//     #[cfg(target_os = "linux")]
//     target_items.extend(items_lin);
//     #[cfg(target_os = "macos")]
//     target_items.extend(items_mac);
//
//     for (file, keyword, max_bytes, action) in target_items {
//         chk_file_content(&mut actions, &file, &keyword, max_bytes, action).await;
//     }
//
//     if !actions.is_empty() {
//         env.lock().await.add_all(actions);
//     }
// }
//
//
// #[cfg(test)]
// mod tests {
//     use crate::sandbox::env::check_env;
//     use super::*;
//     #[tokio::test]
//     async fn sandbox_fs_test() {
//         let env = Environment::new();
//
//         check_folders(env.clone()).await;
//         check_files(env.clone()).await;
//         check_file_totals(env.clone()).await;
//         check_file_contents(env.clone()).await;
//         check_env(env.clone()).await;
//
//         println!("{}", env.lock().await.dump_report());
//     }
// }