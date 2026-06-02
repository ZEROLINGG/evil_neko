// lib/src/sandbox/fs.rs
#![allow(unused)]

use std::env as std_env;
use tokio::fs as tokio_fs;
use anyhow::{anyhow, ensure, Result};
use std::sync::LazyLock;
use tokio::io::{AsyncBufReadExt, BufReader};
use crate::sandbox::*;
use crate::{s, ss}; // 字符串混淆。 s!得到的是String,ss!得到的是&str

static HOME: LazyLock<String> = LazyLock::new(|| {
    std_env::var("HOME")
        .or_else(|_| std_env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/".to_string())
});

static APPDATA: LazyLock<String> = LazyLock::new(|| {
    std_env::var("APPDATA")
        .unwrap_or_else(|_| format!(r"{}\AppData\Roaming", HOME.as_str()))
});

static LOCALAPPDATA: LazyLock<String> = LazyLock::new(|| {
    std_env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| format!(r"{}\AppData\Local", HOME.as_str()))
});

static SYS_DRIVE: LazyLock<String> = LazyLock::new(|| {
    std_env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string())
});

static PROG_FILES: LazyLock<String> = LazyLock::new(|| {
    std_env::var("ProgramFiles").unwrap_or_else(|_| format!(r"{}\Program Files", SYS_DRIVE.as_str()))
});

static PROG_FILES_X86: LazyLock<String> = LazyLock::new(|| {
    std_env::var("ProgramFiles(x86)").unwrap_or_else(|_| format!(r"{}\Program Files (x86)", SYS_DRIVE.as_str()))
});

static PROGRAMDATA: LazyLock<String> = LazyLock::new(|| {
    std_env::var("ProgramData").unwrap_or_else(|_| format!(r"{}\ProgramData", SYS_DRIVE.as_str()))
});
static WINDIR: LazyLock<String> = LazyLock::new(|| {
    std_env::var("windir").unwrap_or_else(|_| format!(r"{}\Windows", SYS_DRIVE.as_str()))
});


pub async fn check_folders(env: &mut Environment) {
    macro_rules! chk_dir {
        ($path:expr, $category:ident, $key:expr, $score:expr, $confidence:expr) => {
            let p = $path;
            if let Ok(meta) = tokio_fs::metadata(&p).await {
                if meta.is_dir() {
                    env.$category(
                        $key,
                        ScoreType::Directory,
                        format!("Directory found: {}", p),
                        $score,
                        $confidence,
                    );
                }
            }
        };
    }

    // ==========================================
    // 1. 风险与对抗目录 (Sandbox / Analysis / VM)
    // ==========================================

    // Windows 常见沙箱/分析目录
    chk_dir!(s!(r#"C:\sandbox"#),               sandbox, SandboxType::Unknown,    10, 0.9);
    chk_dir!(s!(r#"C:\Sandbox"#),               sandbox, SandboxType::Unknown,    10, 0.9);
    chk_dir!(s!(r#"C:\sandboxes"#),             sandbox, SandboxType::Unknown,    10, 0.9);
    chk_dir!(s!(r#"C:\cuckoo"#),                sandbox, SandboxType::Cuckoo,     10, 0.9);
    chk_dir!(s!(r#"C:\cape"#),                  sandbox, SandboxType::CAPE,       10, 0.9);
    chk_dir!(s!(r#"C:\zenbox"#),                sandbox, SandboxType::Zenbox,      9, 0.9);
    chk_dir!(s!(r#"C:\Program Files\Sandbox"#), sandbox, SandboxType::Unknown,    5, 0.8);
    chk_dir!(s!(r#"C:\Program Files\JoeSandbox"#), sandbox, SandboxType::JoeSandbox, 5, 0.8);

    // 恶意软件分析/抓包/逆向工具目录
    chk_dir!(s!(r#"C:\malware"#),               software, SoftwareType::Analysis,  4, 0.5);
    chk_dir!(s!(r#"C:\samples"#),               software, SoftwareType::Analysis,  4, 0.5);
    chk_dir!(s!(r#"C:\VirusTotal"#),            software, SoftwareType::Security,  4, 0.2);
    chk_dir!(format!(r"{}\Wireshark", PROG_FILES.as_str()), software, SoftwareType::Analysis, 6, 0.8);
    chk_dir!(format!(r"{}\Fiddler", LOCALAPPDATA.as_str()), software, SoftwareType::Analysis, 6, 0.8);
    chk_dir!(s!(r#"C:\tools"#),                 software, SoftwareType::Analysis,  1, 0.1);

    // 虚拟机增强组件目录
    chk_dir!(format!(r"{}\VMware\VMware Tools", PROG_FILES.as_str()), virtual_machine, VirtualMachineType::VMware, 9, 0.9);
    chk_dir!(format!(r"{}\Oracle\VirtualBox Guest Additions", PROG_FILES.as_str()), virtual_machine, VirtualMachineType::VirtualBox, 9, 0.9);

    // Linux 常见分析环境目录
    chk_dir!(s!("/opt/cuckoo"),                 sandbox, SandboxType::Cuckoo,     10, 0.9);
    chk_dir!(s!("/opt/cape"),                   sandbox, SandboxType::CAPE,       10, 0.9);
    chk_dir!(s!("/home/sandbox"),               sandbox, SandboxType::Unknown,     9, 0.9);
    chk_dir!(s!("/home/malware"),               software, SoftwareType::Analysis,  6, 0.5);


    chk_dir!(s!(r#"C:\Program Files\CrowdStrike"#), software, SoftwareType::Security, 8, 0.9);
    chk_dir!(s!(r#"C:\Program Files\SentinelOne"#), software, SoftwareType::Security, 8, 0.9);
    chk_dir!(s!(r#"C:\ProgramData\Microsoft\Windows Defender\AdvancedThreatProtection"#), software, SoftwareType::Security, 8, 0.9); // MDE
    chk_dir!(s!(r#"C:\Program Files\FireEye\xagt"#), software, SoftwareType::Security, 8, 0.9);

    chk_dir!(format!(r"{}\SystemInformer", PROG_FILES.as_str()), software, SoftwareType::Analysis, 8, 0.8);
    chk_dir!(s!(r#"C:\fakenet"#), software, SoftwareType::Analysis, 9, 0.9); // 假网络模拟环境
    chk_dir!(s!("/var/lib/kubelet"), container, ContainerType::Kubernetes, 8, 0.8);


    // ==========================================
    // 2. 真实用户可信痕迹目录 (Trust)
    // ==========================================

    // --- 游戏平台 (沙箱几乎不可能安装大型游戏平台) ---
    chk_dir!(format!("{}/.steam", HOME.as_str()),                       trust, TrustType::Game, 10, 0.95);
    chk_dir!(format!("{}/Library/Application Support/Steam", HOME.as_str()), trust, TrustType::Game, 10, 0.95);
    chk_dir!(format!(r"{}\Steam\steamapps\common", PROG_FILES_X86.as_str()), trust, TrustType::Game, 10, 0.95);
    chk_dir!(s!(r#"D:\SteamLibrary\steamapps\common"#),                 trust, TrustType::Game, 10, 0.85);
    chk_dir!(format!(r"{}\Epic Games", PROG_FILES.as_str()),            trust, TrustType::Game,  8, 0.90);
    chk_dir!(format!(r"{}\WeGame", PROG_FILES_X86.as_str()),            trust, TrustType::Game,  8, 0.90);
    chk_dir!(format!("{}/.minecraft", HOME.as_str()),                   trust, TrustType::Game,  8, 0.85);
    chk_dir!(s!(r#"C:\Riot Games"#),                                    trust, TrustType::Game,  8, 0.85);

    // --- 日常社交与办公软件  ---
    chk_dir!(format!("{}/Documents/WeChat Files", HOME.as_str()),       trust, TrustType::InstalledSoftware, 9, 0.95);
    chk_dir!(format!("{}/Documents/Tencent Files", HOME.as_str()),      trust, TrustType::InstalledSoftware, 8, 0.90);
    chk_dir!(format!("{}/DingTalk", APPDATA.as_str()),                  trust, TrustType::InstalledSoftware, 8, 0.85);
    chk_dir!(format!("{}/Feishu", LOCALAPPDATA.as_str()),               trust, TrustType::InstalledSoftware, 8, 0.85);
    chk_dir!(format!("{}/Telegram Desktop", APPDATA.as_str()),          trust, TrustType::InstalledSoftware, 8, 0.90);
    chk_dir!(format!("{}/discord", APPDATA.as_str()),                   trust, TrustType::InstalledSoftware, 8, 0.90);

    // --- 云同步盘 ---
    chk_dir!(format!("{}/OneDrive", HOME.as_str()),                     trust, TrustType::CloudSync, 9, 0.90);
    chk_dir!(format!("{}/Dropbox", HOME.as_str()),                      trust, TrustType::CloudSync, 8, 0.85);
    chk_dir!(format!("{}/Nutstore", HOME.as_str()),                     trust, TrustType::CloudSync, 8, 0.85); // 坚果云
    chk_dir!(format!("{}/iCloudDrive", HOME.as_str()),                  trust, TrustType::CloudSync, 8, 0.90);

    // --- 开发者画像  ---
    chk_dir!(format!("{}/.ssh", HOME.as_str()),                         trust, TrustType::Development, 5, 0.8);
    chk_dir!(format!("{}/.aws", HOME.as_str()),                         trust, TrustType::Development, 8, 0.90);
    chk_dir!(format!("{}/.kube", HOME.as_str()),                        trust, TrustType::Development, 8, 0.90);
    chk_dir!(format!("{}/.cargo", HOME.as_str()),                       trust, TrustType::Development, 7, 0.85);
    chk_dir!(format!("{}/Android", HOME.as_str()),                      trust, TrustType::Development, 7, 0.85);
    chk_dir!(format!("{}/.gradle", HOME.as_str()),                      trust, TrustType::Development, 7, 0.80);
    chk_dir!(format!("{}/.m2", HOME.as_str()),                          trust, TrustType::Development, 6, 0.80);
    chk_dir!(format!("{}/npm", APPDATA.as_str()),                       trust, TrustType::Development, 7, 0.85);

    // --- 浏览器与邮件 ---
    chk_dir!(format!("{}/Google/Chrome/User Data/Default", LOCALAPPDATA.as_str()), trust, TrustType::Browser, 8, 0.95);
    chk_dir!(format!("{}/Microsoft/Edge/User Data/Default", LOCALAPPDATA.as_str()),trust, TrustType::Browser, 7, 0.90);
    chk_dir!(format!("{}/.mozilla/firefox", HOME.as_str()),                  trust, TrustType::Browser, 7, 0.90);
    chk_dir!(format!("{}/Microsoft/Outlook", LOCALAPPDATA.as_str()),         trust, TrustType::EmailClient,    8, 0.90);
    chk_dir!(s!(r#"C:\Foxmail 7.2\Storage"#),                                trust, TrustType::EmailClient,    9, 0.85);
}

pub async fn check_files(env: &mut Environment) {
    macro_rules! chk_file {
        ($path:expr, $category:ident, $key:expr, $score:expr, $confidence:expr) => {
            let p = $path;
            if let Ok(meta) = tokio_fs::symlink_metadata(&p).await {
                if meta.is_file() {
                    env.$category(
                        $key,
                        ScoreType::File,
                        format!("File found: {}", p),
                        $score,
                        $confidence,
                    );
                } else if meta.file_type().is_symlink() {
                    env.$category(
                        $key,
                        ScoreType::File,
                        format!("Honeypot symlink detected: {}", p),
                        10,
                        0.95,
                    );
                }
            }
        };
    }

    let drive = SYS_DRIVE.as_str();
    let pf = PROG_FILES.as_str();
    let pf86 = PROG_FILES_X86.as_str();

    // ==========================================
    // 1. 风险与对抗文件 (Sandbox / Analysis / VM)
    // ==========================================

    // === Windows: 沙箱专属文件/Agent ===
    chk_file!(format!(r"{}\Windows\System32\cuckoomon.dll", drive), sandbox, SandboxType::Cuckoo, 10, 0.9);
    chk_file!(format!(r"{}\cuckoomon.dll", drive),                  sandbox, SandboxType::Cuckoo, 10, 0.9);
    chk_file!(format!(r"{}\capemon.dll", drive),                    sandbox, SandboxType::CAPE,   10, 0.9);
    chk_file!(format!(r"{}\agent.py", drive),                       sandbox, SandboxType::Unknown, 9, 0.8);
    chk_file!(format!(r"{}\agent.pyw", drive),                      sandbox, SandboxType::Unknown, 9, 0.8);

    // === Windows: 虚拟化平台驱动与进程 ===
    chk_file!(format!(r"{}\Windows\System32\drivers\VBoxGuest.sys", drive), virtual_machine, VirtualMachineType::VirtualBox, 9, 0.8);
    chk_file!(format!(r"{}\Windows\System32\drivers\VBoxMouse.sys", drive), virtual_machine, VirtualMachineType::VirtualBox, 9, 0.8);
    chk_file!(format!(r"{}\Windows\System32\drivers\vmhgfs.sys", drive),    virtual_machine, VirtualMachineType::VMware,     9, 0.8);
    chk_file!(format!(r"{}\Windows\System32\drivers\vmmouse.sys", drive),   virtual_machine, VirtualMachineType::VMware,     9, 0.8);
    chk_file!(format!(r"{}\Windows\System32\drivers\prl_tg.sys", drive),    virtual_machine, VirtualMachineType::Parallels,  8, 0.8);

    // === Windows: 监控、分析与调试工具扩展 ===
    chk_file!(format!(r"{}\Sysinternals\ProcMon.exe", pf),          software, SoftwareType::Analysis, 5, 0.4);
    chk_file!(format!(r"{}\Process Hacker 2\ProcessHacker.exe", pf),software, SoftwareType::Analysis, 6, 0.5);
    chk_file!(format!(r"{}\x64dbg\release\x64dbg.exe", drive),      software, SoftwareType::Debugger, 8, 0.7);
    chk_file!(format!(r"{}\x64dbg\release\x32dbg.exe", drive),      software, SoftwareType::Debugger, 8, 0.7);
    chk_file!(format!(r"{}\Ghidra\ghidraRun.bat", drive),           software, SoftwareType::Debugger, 7, 0.6);
    chk_file!(format!(r"{}\IDA\ida64.exe", pf86),                   software, SoftwareType::Debugger, 7, 0.6);
    chk_file!(format!(r"{}\Windows\System32\drivers\npcap.sys", drive), software, SoftwareType::Analysis, 6, 0.6); // 抓包驱动

    // === Linux/容器: 环境指纹 ===
    chk_file!(s!("/.dockerenv"),                             container, ContainerType::Docker, 7, 0.8);
    chk_file!(s!("/run/.containerenv"),                      container, ContainerType::Podman, 7, 0.8);
    chk_file!(s!("/run/docker.sock"),                        container, ContainerType::Docker, 8, 0.9);
    chk_file!(s!("/opt/cuckoo/agent.py"),                    sandbox, SandboxType::Cuckoo,    10, 0.9);
    chk_file!(s!("/usr/sbin/VBoxService"),                   virtual_machine, VirtualMachineType::VirtualBox, 8, 0.8);


    chk_file!(format!(r"{}\Windows\System32\drivers\viocrypt.sys", drive), virtual_machine, VirtualMachineType::KVM, 9, 0.8);
    chk_file!(format!(r"{}\Windows\System32\drivers\vioscsi.sys", drive), virtual_machine, VirtualMachineType::KVM, 9, 0.8);
    chk_file!(format!(r"{}\Windows\System32\drivers\vmmouse.sys", drive), virtual_machine, VirtualMachineType::VMware, 9, 0.8);
    chk_file!(format!(r"{}\Windows\System32\drivers\vmci.sys", drive), virtual_machine, VirtualMachineType::VMware, 9, 0.8);
    chk_file!(format!(r"{}\Windows\System32\drivers\balloon.sys", drive), virtual_machine, VirtualMachineType::VMware, 8, 0.7);
    chk_file!(s!("/dev/virtio-ports"), virtual_machine, VirtualMachineType::KVM, 8, 0.8);
    chk_file!(format!(r"{}\Windows\System32\drivers\vmbus.sys", drive), virtual_machine, VirtualMachineType::HyperV, 9, 0.8);
    chk_file!(format!(r"{}\Windows\System32\drivers\hypervideo.sys", drive), virtual_machine, VirtualMachineType::HyperV, 9, 0.8);
    chk_file!(s!("/Library/Frameworks/ParallelsDesktop.framework"), virtual_machine, VirtualMachineType::Parallels, 9, 0.9);
    chk_file!(format!(r"{}\agent.exe", drive), sandbox, SandboxType::Cuckoo, 10, 0.9);
    chk_file!(format!(r"{}\analyzer.py", drive), sandbox, SandboxType::Unknown, 9, 0.8);
    chk_file!(s!(r#"C:\analysis.log"#), sandbox, SandboxType::Unknown, 8, 0.8);
    chk_file!(s!(r#"C:\sandbox.log"#), sandbox, SandboxType::Unknown, 8, 0.8);
    chk_file!(s!("/var/run/containerd/containerd.sock"), container, ContainerType::Containerd, 8, 0.8);
    chk_file!(s!("/var/run/crio/crio.sock"), container, ContainerType::Unknown, 7, 0.7);
    chk_file!(s!("/var/run/secrets/kubernetes.io"), container, ContainerType::Kubernetes, 9, 0.9);
    chk_file!(s!("/proc/sys/fs/binfmt_misc/WSLInterop"), container, ContainerType::Wsl, 9, 0.9);



    // ==========================================
    // 2. 真实用户可信痕迹文件 (Trust)
    // ==========================================

    // --- 浏览器历史数据库  ---
    chk_file!(format!("{}/Google/Chrome/User Data/Default/History", LOCALAPPDATA.as_str()), trust, TrustType::Browser, 8, 0.90);
    chk_file!(format!("{}/Google/Chrome/User Data/Default/Login Data", LOCALAPPDATA.as_str()), trust, TrustType::Browser, 9, 0.95);
    chk_file!(format!("{}/Microsoft/Edge/User Data/Default/History", LOCALAPPDATA.as_str()), trust, TrustType::Browser, 8, 0.85);

    // --- 敏感配置与密钥 (开发者/IT人员机器特有，极高置信度) ---
    chk_file!(format!("{}/.ssh/id_rsa", HOME.as_str()),                 trust, TrustType::Development, 10, 0.95);
    chk_file!(format!("{}/.ssh/id_ed25519", HOME.as_str()),             trust, TrustType::Development, 10, 0.95);
    chk_file!(format!("{}/.ssh/known_hosts", HOME.as_str()),            trust, TrustType::Development,  8, 0.90); // 长期连接服务器的证据
    chk_file!(format!("{}/.gitconfig", HOME.as_str()),                  trust, TrustType::Development,  8, 0.85);
    chk_file!(format!("{}/.aws/credentials", HOME.as_str()),            trust, TrustType::Development,  9, 0.95);
    chk_file!(format!("{}/.kube/config", HOME.as_str()),                trust, TrustType::Development,  9, 0.95);

    // --- 终端历史记录 (Linux/Mac 真实人类操作证据) ---
    chk_file!(format!("{}/.bash_history", HOME.as_str()),               trust, TrustType::PersonalFiles, 3, 0.80);
    chk_file!(format!("{}/.zsh_history", HOME.as_str()),                trust, TrustType::PersonalFiles, 3, 0.80);
    chk_file!(format!("{}/.psql_history", HOME.as_str()),               trust, TrustType::PersonalFiles, 5, 0.85); // 数据库操作历史

    // --- 个人/系统长期使用痕迹文件 ---
    chk_file!(format!("{}/ntuser.dat", HOME.as_str()),                  trust, TrustType::UserAccounts, 5, 0.60); // Windows用户配置库
    // 微信/Telegram 通常会在本地生成特定的数据库或 session 文件
    chk_file!(format!("{}/Telegram Desktop/tdata/key_datas", APPDATA.as_str()), trust, TrustType::InstalledSoftware, 9, 0.95);
}



pub async fn check_file_totals(env: &mut Environment) {
    /// 检测目录文件数量过少（沙箱环境典型特征）
    async fn check_too_few(
        env: &mut Environment,
        dir: &str,
        min: u32,
        score: u8,
        confidence: f32
    ) {
        if let Ok(mut entries) = tokio_fs::read_dir(dir).await {
            let mut count: u32 = 0;
            while let Ok(Some(_)) = entries.next_entry().await {
                count += 1;
                if count >= min { return; }
            }
            env.sandbox(
                SandboxType::Unknown,
                ScoreType::UserActivity,
                format!("Lack of user activity in {}: count={}", dir, count),
                score,
                confidence,
            );
        }
    }

    /// 检测目录文件数量充足（真实环境可信证据）
    async fn check_sufficient(
        env: &mut Environment,
        dir: &str,
        target: TrustType,
        min_threshold: u32,
        score: u8,
        confidence: f32,
    ) {
        if let Ok(mut entries) = tokio_fs::read_dir(dir).await {
            let mut count: u32 = 0;
            while let Ok(Some(_)) = entries.next_entry().await {
                count += 1;
                if count >= min_threshold {
                    env.trust(
                        target,
                        ScoreType::UserActivity,
                        format!("Rich user activity in {}: count={}+", dir, count),
                        score,
                        confidence,
                    );
                    return;
                }
            }
        }
    }

    /// 检测文件数量并返回精确计数（用于分级评估）
    async fn count_files(dir: &str, max_count: u32) -> u32 {
        let Ok(mut entries) = tokio_fs::read_dir(dir).await else { return 0; };
        let mut count = 0;
        while let Ok(Some(_)) = entries.next_entry().await {
            count += 1;
            if count >= max_count { break; }
        }
        count
    }

    /// 分级可信度评估（根据文件数量动态调整置信度）
    async fn check_tiered_trust(
        env: &mut Environment,
        dir: &str,
        target: TrustType,
        thresholds: &[(u32, u8, f32)], // (min_count, score, confidence)
    ) {
        let count = count_files(dir, thresholds.last().unwrap().0 + 100).await;

        for &(threshold, score, conf) in thresholds.iter().rev() {
            if count >= threshold {
                env.trust(
                    target,
                    ScoreType::UserActivity,
                    format!("Verified activity depth in {}: {} files", dir, count),
                    score,
                    conf,
                );
                return;
            }
        }
    }

    let home = HOME.as_str();
    let appdata = APPDATA.as_str();
    let localappdata = LOCALAPPDATA.as_str();
    let drive = SYS_DRIVE.as_str();

    // ==========================================
    // 核心系统目录：数量过少 = 沙箱特征
    // ==========================================

    // Windows 用户主目录（真实环境通常 >20 个条目）
    check_too_few(env, home, 20, 5, 0.65).await;

    // Windows Recent 文件（真实用户至少 15+ 个最近文件）
    let recent = format!("{}/Microsoft/Windows/Recent", appdata);
    check_too_few(env, &recent, 15, 4, 0.60).await;

    // 临时文件目录（真实环境通常有大量残留）
    let temp_user = format!("{}/Temp", localappdata);
    check_too_few(env, &temp_user, 30, 4, 0.60).await;
    check_too_few(env, &format!(r"{}\Windows\Temp", drive), 30, 4, 0.60).await;

    // Windows Prefetch（预取缓存，真实系统 >50 个）
    check_too_few(env, &format!(r"{}\Windows\Prefetch", drive), 50, 5, 0.70).await;

    // Windows 字体目录（真实系统 >150 个字体文件）
    check_too_few(env, &format!(r"{}\Windows\Fonts", drive), 150, 4, 0.65).await;

    // 已安装程序目录（真实环境至少 10+ 个软件）
    check_too_few(env, PROG_FILES.as_str(), 10, 4, 0.60).await;
    check_too_few(env, PROG_FILES_X86.as_str(), 10, 4, 0.60).await;

    // 开始菜单快捷方式（真实用户 >30 个程序入口）
    let start_menu = format!("{}/Microsoft/Windows/Start Menu/Programs", appdata);
    check_too_few(env, &start_menu, 30, 4, 0.60).await;

    // Linux/macOS 关键目录
    check_too_few(env, "/tmp", 20, 4, 0.60).await;
    check_too_few(env, "/var/log", 15, 4, 0.60).await;
    check_too_few(env, "/usr/bin", 500, 5, 0.70).await; // 真实 Linux 系统至少 500+ 个二进制
    check_too_few(env, "/Applications", 10, 4, 0.60).await; // macOS 应用数量


    check_too_few(env, r#"C:\$Recycle.Bin"#, 5, 5, 0.70).await;

    // ==========================================
    // 浏览器数据：数量丰富 = 强可信证据
    // ==========================================

    // Chrome 浏览器扩展（真实用户通常安装 5+ 个扩展）
    let chrome_ext = format!("{}/Google/Chrome/User Data/Default/Extensions", localappdata);
    check_tiered_trust(env, &chrome_ext, TrustType::Browser, &[
        (5,  6, 0.70),
        (10, 7, 0.80),
        (20, 8, 0.90),
    ]).await;

    // Chrome 历史记录数据库大小
    let chrome_history = format!("{}/Google/Chrome/User Data/Default", localappdata);
    check_tiered_trust(env, &chrome_history, TrustType::Browser, &[
        (15, 7, 0.80),
        (30, 8, 0.90),
        (50, 9, 0.95),
    ]).await;

    // Edge 浏览器数据
    let edge_data = format!("{}/Microsoft/Edge/User Data/Default", localappdata);
    check_tiered_trust(env, &edge_data, TrustType::Browser, &[
        (15, 6, 0.75),
        (30, 7, 0.85),
    ]).await;

    // Firefox 配置文件（真实用户至少 1 个 profile）
    let firefox_profiles = format!("{}/.mozilla/firefox", home);
    check_sufficient(env, &firefox_profiles, TrustType::Browser, 1, 7, 0.80).await;

    // ==========================================
    // 游戏平台：文件数量 = 游戏库深度
    // ==========================================

    // Steam 游戏库
    let steam_common = format!(r"{}\Steam\steamapps\common", PROG_FILES_X86.as_str());
    check_tiered_trust(env, &steam_common, TrustType::Game, &[
        (3,   7, 0.75),
        (10,  8, 0.85),
        (30,  9, 0.92),
        (100, 10, 0.98),
    ]).await;

    // Epic Games 库
    let epic_games = format!(r"{}\Epic Games", PROG_FILES.as_str());
    check_tiered_trust(env, &epic_games, TrustType::Game, &[
        (2, 7, 0.75),
        (5, 8, 0.85),
        (10, 9, 0.90),
    ]).await;

    // WeGame 游戏库（D盘常见安装位置）
    check_tiered_trust(env, s!("D:\\WeGamePlatform").as_str(), TrustType::Game, &[
        (3, 7, 0.75),
        (10, 8, 0.85),
    ]).await;

    // ==========================================
    // 4. 开发环境：工具链文件数量
    // ==========================================

    // Cargo 缓存
    let cargo_registry = format!("{}/.cargo/registry/cache", home);
    check_tiered_trust(env, &cargo_registry, TrustType::Development, &[
        (10,  6, 0.70),
        (50,  7, 0.80),
        (200, 8, 0.90),
    ]).await;

    // npm 全局包（Node.js 开发环境）
    let npm_global = format!("{}/npm", appdata);
    check_tiered_trust(env, &npm_global, TrustType::Development, &[
        (10, 6, 0.70),
        (30, 7, 0.80),
        (100, 8, 0.90),
    ]).await;

    // Python pip 缓存
    let pip_cache = format!("{}/pip", localappdata);
    check_tiered_trust(env, &pip_cache, TrustType::Development, &[
        (10, 6, 0.70),
        (50, 7, 0.80),
    ]).await;

    // Gradle 缓存（Android/Java 开发）
    let gradle_cache = format!("{}/.gradle/caches/modules-2/files-2.1", home);
    check_tiered_trust(env, &gradle_cache, TrustType::Development, &[
        (50, 7, 0.80),
        (200, 8, 0.90),
    ]).await;

    // VS Code 扩展数量
    let vscode_ext = format!("{}/.vscode/extensions", home);
    check_tiered_trust(env, &vscode_ext, TrustType::Development, &[
        (5, 6, 0.75),
        (15, 7, 0.85),
        (30, 8, 0.90),
    ]).await;

    // ==========================================
    //  IM 与办公软件：聊天记录深度
    // ==========================================

    // 微信聊天记录文件数量（FileStorage 目录）
    let wechat_files = format!("{}/Documents/WeChat Files", home);
    check_tiered_trust(env, &wechat_files, TrustType::InstalledSoftware, &[
        (10,  7, 0.80),
        (50,  8, 0.90),
        (200, 9, 0.95),
    ]).await;

    // QQ 聊天记录
    let qq_files = format!("{}/Documents/Tencent Files", home);
    check_tiered_trust(env, &qq_files, TrustType::InstalledSoftware, &[
        (10, 7, 0.80),
        (50, 8, 0.90),
    ]).await;

    // ==========================================
    // 个人文件目录：文档数量
    // ==========================================

    // Documents 目录
    let documents = format!("{}/Documents", home);
    check_tiered_trust(env, &documents, TrustType::PersonalFiles, &[
        (20,  5, 0.60),
        (50,  6, 0.70),
        (100, 7, 0.80),
    ]).await;

    // Downloads 目录（
    let downloads = format!("{}/Downloads", home);
    check_tiered_trust(env, &downloads, TrustType::PersonalFiles, &[
        (30,  5, 0.60),
        (100, 6, 0.70),
        (300, 7, 0.80),
    ]).await;

    // Pictures 目录（照片库）
    let pictures = format!("{}/Pictures", home);
    check_tiered_trust(env, &pictures, TrustType::PersonalFiles, &[
        (50,  6, 0.70),  // 50+ 照片：基础使用
        (200, 7, 0.80),  // 200+ 照片：正常使用
        (1000, 8, 0.90), // 1000+ 照片：摄影爱好者/长期积累
    ]).await;

    // Desktop 目录（桌面文件数量）
    let desktop = format!("{}/Desktop", home);
    check_tiered_trust(env, &desktop, TrustType::PersonalFiles, &[
        (10, 5, 0.60),
        (30, 6, 0.70),
    ]).await;

    // ==========================================
    // 邮件客户端：邮件数据库复杂度
    // ==========================================

    // Outlook 数据文件（PST/OST 附属文件）
    let outlook_data = format!("{}/Microsoft/Outlook", localappdata);
    check_tiered_trust(env, &outlook_data, TrustType::EmailClient, &[
        (3, 7, 0.80),
        (10, 8, 0.90),
    ]).await;

    // Foxmail 邮件存储（账号数量）
    check_tiered_trust(env, s!(r#"C:\Foxmail 7.2\Storage"#).as_str(), TrustType::EmailClient, &[
        (2, 7, 0.80),
        (5, 8, 0.90),
    ]).await;

    // Thunderbird 配置文件
    let thunderbird = format!("{}/Thunderbird/Profiles", appdata);
    check_sufficient(env, &thunderbird, TrustType::EmailClient, 1, 7, 0.80).await;

    // ==========================================
    // 8. 云同步服务：同步文件数量
    // ==========================================

    // OneDrive 同步文件数量
    let onedrive = format!("{}/OneDrive", home);
    check_tiered_trust(env, &onedrive, TrustType::CloudSync, &[
        (20,  7, 0.80),
        (100, 8, 0.90),
        (500, 9, 0.95),
    ]).await;

    // Dropbox 同步文件
    let dropbox = format!("{}/Dropbox", home);
    check_tiered_trust(env, &dropbox, TrustType::CloudSync, &[
        (20, 7, 0.80),
        (100, 8, 0.90),
    ]).await;

    // 坚果云同步文件
    let nutstore = format!("{}/Nutstore", home);
    check_tiered_trust(env, &nutstore, TrustType::CloudSync, &[
        (20, 7, 0.80),
        (100, 8, 0.90),
    ]).await;

    // ==========================================
    // 系统日志与事件：历史积累深度
    // ==========================================

    // Windows 事件日志文件数量
    let event_logs = format!(r"{}\System32\winevt\Logs", WINDIR.as_str());
    check_tiered_trust(env, &event_logs, TrustType::EventLogs, &[
        (50,  6, 0.70),
        (100, 7, 0.80),
        (200, 8, 0.90),
    ]).await;

    // Linux 系统日志
    check_tiered_trust(env, "/var/log", TrustType::EventLogs, &[
        (20, 6, 0.70),
        (50, 7, 0.80),
        (100, 8, 0.90),
    ]).await;

    // ==========================================
    // 网络配置：Wi-Fi 已知网络数量
    // ==========================================

    // Windows Wi-Fi 配置文件（每个保存的网络对应 1 个 XML）
    let wifi_profiles = format!(r"{}\Microsoft\Wlansvc\Profiles\Interfaces", PROGRAMDATA.as_str());
    check_tiered_trust(env, &wifi_profiles, TrustType::Network, &[
        (3,  6, 0.70),
        (10, 7, 0.85),
        (20, 8, 0.92),
    ]).await;

    // macOS Wi-Fi 偏好设置
    let macos_wifi = format!("{}/Library/Preferences/com.apple.wifi.known-networks.plist", home);
    if let Ok(meta) = tokio_fs::metadata(&macos_wifi).await {
        if meta.len() > 5000 { // 大文件 = 大量网络历史
            env.trust(
                TrustType::Network,
                ScoreType::UserActivity,
                format!("macOS known WiFi networks file size: {} bytes", meta.len()),
                7, 0.85
            );
        }
    }

    // ==========================================
    // 11. 注册表使用深度（Windows）
    // ==========================================

    // 用户注册表 Hive 文件大小（通过目录文件数量间接推断）
    let user_reg = format!("{}/Microsoft/Windows", localappdata);
    check_tiered_trust(env, &user_reg, TrustType::RegistryUsage, &[
        (50,  5, 0.60),
        (100, 6, 0.70),
        (200, 7, 0.80),
    ]).await;

    // ==========================================
    // 12. 已安装字体数量（真实系统特征）
    // ==========================================

    let fonts_dir = format!(r"{}\Windows\Fonts", drive);
    let font_count = count_files(&fonts_dir, 500).await;
    if font_count >= 200 {
        env.trust(
            TrustType::InstalledSoftware,
            ScoreType::UserActivity,
            format!("Rich font installation: {} fonts installed", font_count),
            6, 0.75
        );
    }

    // ==========================================
    // 13. 已安装软件快捷方式数量
    // ==========================================

    // 开始菜单所有用户程序
    let all_users_start = format!(r"{}\Microsoft\Windows\Start Menu\Programs", PROGRAMDATA.as_str());
    check_tiered_trust(env, &all_users_start, TrustType::InstalledSoftware, &[
        (20, 6, 0.70),
        (50, 7, 0.80),
        (100, 8, 0.90),
    ]).await;

    // 用户个人程序快捷方式
    let user_start = format!("{}/Microsoft/Windows/Start Menu/Programs", appdata);
    check_tiered_trust(env, &user_start, TrustType::InstalledSoftware, &[
        (10, 5, 0.65),
        (30, 6, 0.75),
    ]).await;


    let ssh_known = format!("{}/.ssh/known_hosts", home);
    if let Ok(content) = tokio_fs::read_to_string(&ssh_known).await {
        let host_count = content.lines().count();
        if host_count >= 5 {
            env.trust(
                TrustType::Network,
                ScoreType::FileContent,
                format!("SSH known hosts: {} entries", host_count),
                7, 0.85
            );
        }
    }

    let usb_devices = count_files("/sys/bus/usb/devices", 50).await;
    if usb_devices > 5 {
        env.trust(
            TrustType::PhysicalDevices,
            ScoreType::UserActivity,
            format!("Multiple USB devices: {}", usb_devices),
            7, 0.8
        );
    }
}

pub async fn check_file_contents(env: &mut Environment) {
    /// 增强版 file_contains：自动过滤 Null 字节以兼容 UTF-16LE (Windows常用)
    async fn file_contains(file: &str, keyword: &str, max_bytes: u64) -> bool {
        let Ok(f) = tokio_fs::File::open(file).await else { return false; };
        let mut reader = BufReader::new(f);
        let mut line = Vec::with_capacity(512);
        let mut read = 0u64;
        let needle = keyword.to_lowercase();

        loop {
            if max_bytes > 0 && read >= max_bytes { break; }
            line.clear();
            match reader.read_until(b'\n', &mut line).await {
                Ok(0) | Err(_) => break,
                Ok(n) => read += n as u64,
            }

            // 将读取的字节转换为字符串，并过滤掉 \x00 (零字节)
            // 这是一个轻量级的 Hack，能让 ASCII 关键字在 UTF-16LE 文件中也能被成功匹配
            let mut raw_str = String::from_utf8_lossy(&line).into_owned();
            raw_str.retain(|c| c != '\0');

            if raw_str.to_lowercase().contains(&needle) {
                return true;
            }
        }
        false
    }

    macro_rules! chk_content {
        ($file:expr, $keyword:expr, $max_bytes:expr, $category:ident, $key:expr, $score_type:ident, $score:expr, $confidence:expr) => {
            if file_contains($file, $keyword, $max_bytes).await {
                env.$category(
                    $key,
                    ScoreType::$score_type,
                    format!("{} contains '{}'", $file, $keyword),
                    $score,
                    $confidence,
                );
            }
        };
    }

    const KB: u64 = 1024;
    let home = HOME.as_str();
    let sys_drive = SYS_DRIVE.as_str();
    let appdata = APPDATA.as_str();

    // ==========================================
    //  风险与对抗：Linux VM / 沙箱指纹 (Risk)
    // ==========================================

    // === /proc/cpuinfo ===
    chk_content!("/proc/cpuinfo", "hypervisor",    64*KB, virtual_machine, VirtualMachineType::Unknown, Cpu,  8, 0.8);
    chk_content!("/proc/cpuinfo", "KVMKVMKVM",     64*KB, virtual_machine, VirtualMachineType::KVM,     Cpu, 10, 0.9);
    chk_content!("/proc/cpuinfo", "VMwareVMware",  64*KB, virtual_machine, VirtualMachineType::VMware,  Cpu, 10, 0.9);
    chk_content!("/proc/cpuinfo", "XenVMMXenVMM",  64*KB, virtual_machine, VirtualMachineType::Xen,     Cpu, 10, 0.9);
    chk_content!("/proc/cpuinfo", "Microsoft Hv",  64*KB, virtual_machine, VirtualMachineType::HyperV,  Cpu,  9, 0.8);

    // === /proc/version ===
    chk_content!("/proc/version", "cuckoo",         4*KB, sandbox,         SandboxType::Cuckoo,  FileContent, 10, 0.9);
    chk_content!("/proc/version", "sandbox",        4*KB, sandbox,         SandboxType::Unknown, FileContent,  8, 0.8);

    // === /proc/1/cgroup ===
    chk_content!("/proc/1/cgroup", "docker",       16*KB, container,       ContainerType::Docker,     FileContent, 8, 0.8);
    chk_content!("/proc/1/cgroup", "kubepods",     16*KB, container,       ContainerType::Kubernetes, FileContent, 7, 0.7);
    chk_content!("/proc/1/cgroup", "lxc",          16*KB, container,       ContainerType::LXC,        FileContent, 8, 0.8);

    // === /proc/scsi/scsi & mounts & net/dev ===
    chk_content!("/proc/scsi/scsi", "VBOX",        32*KB, virtual_machine, VirtualMachineType::VirtualBox, FileContent, 9, 0.8);
    chk_content!("/proc/scsi/scsi", "VMWARE",      32*KB, virtual_machine, VirtualMachineType::VMware,     FileContent, 9, 0.8);
    chk_content!("/proc/scsi/scsi", "QEMU",        32*KB, emulator,        EmulatorType::QemuTCG,          FileContent, 9, 0.8);

    chk_content!("/proc/mounts", "vboxsf",         64*KB, virtual_machine, VirtualMachineType::VirtualBox, FileContent, 9, 0.8);
    chk_content!("/proc/mounts", "vmhgfs",         64*KB, virtual_machine, VirtualMachineType::VMware,     FileContent, 9, 0.8);

    chk_content!("/proc/net/dev", "vboxnet",       16*KB, virtual_machine, VirtualMachineType::VirtualBox, Network, 8, 0.8);
    chk_content!("/proc/net/dev", "vmnet",         16*KB, virtual_machine, VirtualMachineType::VMware,     Network, 8, 0.8);

    // === DMI / sysfs (Linux 硬件指纹) ===
    chk_content!("/sys/class/dmi/id/product_name", "VirtualBox",   KB, virtual_machine, VirtualMachineType::VirtualBox, Dmi, 10, 0.9);
    chk_content!("/sys/class/dmi/id/product_name", "VMware",       KB, virtual_machine, VirtualMachineType::VMware,     Dmi, 10, 0.9);
    chk_content!("/sys/class/dmi/id/product_name", "QEMU",         KB, emulator,        EmulatorType::QemuTCG,          Dmi, 10, 0.9);
    chk_content!("/sys/class/dmi/id/sys_vendor",   "innotek GmbH", KB, virtual_machine, VirtualMachineType::VirtualBox, Dmi, 10, 0.9);
    chk_content!("/sys/class/dmi/id/sys_vendor",   "VMware, Inc.", KB, virtual_machine, VirtualMachineType::VMware,     Dmi, 10, 0.9);
    chk_content!("/sys/class/dmi/id/bios_vendor",  "SeaBIOS",      KB, emulator,        EmulatorType::Unknown,          Bios, 7, 0.7);

    // === /var/log/dmesg ===
    chk_content!("/var/log/dmesg", "Hypervisor detected", 512*KB, virtual_machine, VirtualMachineType::Unknown,    Driver, 8, 0.8);
    chk_content!("/var/log/dmesg", "VMware vmxnet",       512*KB, virtual_machine, VirtualMachineType::VMware,     Driver, 9, 0.8);


    chk_content!("/proc/version", "Microsoft", 4*KB, container, ContainerType::Wsl, FileContent, 8, 0.8);
    chk_content!("/proc/cpuinfo", "AuthenticAMD", 64*KB, trust, TrustType::PhysicalDevices, Cpu, 6, 0.7);
    chk_content!("/proc/cpuinfo", "GenuineIntel", 64*KB, trust, TrustType::PhysicalDevices, Cpu, 6, 0.7);

    // ==========================================
    // 风险与对抗：Windows VM / 沙箱指纹 (Risk)
    // ==========================================

    // Windows Hosts
    let hosts_path = format!(r"{}\Windows\System32\drivers\etc\hosts", sys_drive);
    chk_content!(&hosts_path, "cuckoo",        128*KB, sandbox, SandboxType::Cuckoo,  FileContent, 10, 0.9);
    chk_content!(&hosts_path, "virustotal",    128*KB, sandbox, SandboxType::Unknown, FileContent, 8, 0.8);

    // SetupAPI.dev.log
    let setupapi_log = format!(r"{}\Windows\inf\setupapi.dev.log", sys_drive);
    chk_content!(&setupapi_log, "vboxguest",   5120*KB, virtual_machine, VirtualMachineType::VirtualBox, Driver, 9, 0.8);
    chk_content!(&setupapi_log, "vboxvideo",   5120*KB, virtual_machine, VirtualMachineType::VirtualBox, Driver, 9, 0.8);
    chk_content!(&setupapi_log, "vmware",      5120*KB, virtual_machine, VirtualMachineType::VMware,     Driver, 9, 0.8);
    chk_content!(&setupapi_log, "ven_80ee",    5120*KB, virtual_machine, VirtualMachineType::VirtualBox, Driver, 9, 0.85); // VBox Vendor ID

    // ==========================================
    // 真实用户可信痕迹内容扫描 (Trust)
    // ==========================================

    // --- 开发者画像：Git 配置 ---
    let gitconfig = format!("{}/.gitconfig", home);
    chk_content!(&gitconfig, "email = ",       16*KB, trust, TrustType::Development, FileContent, 9, 0.9);
    chk_content!(&gitconfig, "name = ",        16*KB, trust, TrustType::Development, FileContent, 9, 0.9);

    // --- 开发者画像：SSH 配置  ---
    let ssh_config = format!("{}/.ssh/config", home);
    chk_content!(&ssh_config, "HostName ",     32*KB, trust, TrustType::Development, FileContent, 8, 0.85);
    chk_content!(&ssh_config, "IdentityFile ", 32*KB, trust, TrustType::Development, FileContent, 9, 0.9);

    let ssh_known_hosts = format!("{}/.ssh/known_hosts", home);
    chk_content!(&ssh_known_hosts, "ssh-rsa ",       128*KB, trust, TrustType::Network, FileContent, 7, 0.8);
    chk_content!(&ssh_known_hosts, "ecdsa-sha2-",    128*KB, trust, TrustType::Network, FileContent, 7, 0.8);

    // --- 开发者画像：云服务凭证 ---
    let aws_credentials = format!("{}/.aws/credentials", home);
    chk_content!(&aws_credentials, "aws_access_key_id", 16*KB, trust, TrustType::Development, FileContent, 10, 0.95);

    let npmrc = format!("{}/.npmrc", home);
    chk_content!(&npmrc, "registry=",          16*KB, trust, TrustType::Development, FileContent, 7, 0.8);

    // --- 真实用户活动：Linux Bash/Zsh 历史记录 ---
    let bash_history = format!("{}/.bash_history", home);
    chk_content!(&bash_history, "git commit",  256*KB, trust, TrustType::Development, UserActivity, 8, 0.85);
    chk_content!(&bash_history, "ssh ",        256*KB, trust, TrustType::Network,     UserActivity, 7, 0.85);
    chk_content!(&bash_history, "docker run",  256*KB, trust, TrustType::Development, UserActivity, 7, 0.85);
    chk_content!(&bash_history, "sudo apt",    256*KB, trust, TrustType::UserAccounts,UserActivity, 6, 0.7);
    chk_content!(&bash_history, "cd ",    256*KB, trust, TrustType::UserAccounts,UserActivity, 4, 0.6);

    // --- 真实用户活动：Windows PowerShell 历史记录 ---
    let ps_history = format!(r"{}\Microsoft\Windows\PowerShell\PSReadLine\ConsoleHost_history.txt", appdata);
    chk_content!(&ps_history, "git ",          256*KB, trust, TrustType::Development, UserActivity, 8, 0.85);
    chk_content!(&ps_history, "npm ",          256*KB, trust, TrustType::Development, UserActivity, 8, 0.85);
    chk_content!(&ps_history, "ping ",         256*KB, trust, TrustType::Network,     UserActivity, 5, 0.6);
    chk_content!(&ps_history, "ssh ",          256*KB, trust, TrustType::Network,     UserActivity, 8, 0.85);
    chk_content!(&ps_history, "cd ",           256*KB, trust, TrustType::Development, UserActivity, 4, 0.6);



    chk_content!(ss!("/sys/devices/virtual/dmi/id/product_name"), "QEMU", 1*KB, emulator, EmulatorType::QemuTCG,Driver, 9, 0.8);

}



#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn sandbox_fs_test() {
        let mut env = Environment::new();

        check_folders(&mut env).await;
        check_files(&mut env).await;
        check_file_totals(&mut env).await;
        check_file_contents(&mut env).await;

        println!("Risk Score: {:.2}%", env.final_risk_score());
        println!("{}", env.dump_report());
    }
}