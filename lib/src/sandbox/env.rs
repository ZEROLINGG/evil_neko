// lib/src/sandbox/env.rs
#![allow(unused)]

use std::env as std_env;
use crate::sandbox::*;
use crate::{s, ss}; // 字符串混淆宏



/// 环境变量全量分析
pub async fn check_env_vars(env: &mut Environment) {
    // 收集所有环境变量
    let all_vars: Vec<(String, String)> = std_env::vars().collect();
    let env_count = all_vars.len();

    // ==========================================
    // 0. 环境变量复杂度评估 (活动深度)
    // ==========================================
    if env_count < 15 {
        env.sandbox(
            SandboxType::Unknown,
            ScoreType::Env,
            format!("Unusually low environment variables count: {}", env_count),
            5, 0.75,
        );
    } else if env_count > 45 {
        env.trust(
            TrustType::UserAccounts,
            ScoreType::Env,
            format!("Rich and complex environment variables count: {}", env_count),
            6, 0.80,
        );
    }

    // 宏：匹配环境变量的 Key (模糊匹配)
    macro_rules! chk_key {
        ($k:expr, $needle:expr, $category:ident, $type_enum:expr, $score:expr, $conf:expr) => {
            if $k.to_uppercase().contains(&$needle.to_uppercase()) {
                env.$category(
                    $type_enum,
                    ScoreType::Env,
                    format!("Suspicious Env Key found: {}", $k),
                    $score, $conf,
                );
            }
        };
    }

    // 宏：匹配特定环境变量的 Value (模糊匹配)
    macro_rules! chk_val {
        ($k:expr, $v:expr, $target_key:expr, $val_needle:expr, $category:ident, $type_enum:expr, $score:expr, $conf:expr) => {
            if $k.eq_ignore_ascii_case($target_key) && $v.to_lowercase().contains(&$val_needle.to_lowercase()) {
                env.$category(
                    $type_enum,
                    ScoreType::Env,
                    format!("Env [{}] contains risk value: {}", $k, $v),
                    $score, $conf,
                );
            }
        };
    }

    // 宏：匹配可信特征的 Key (精确/前缀匹配)
    macro_rules! trust_key {
        ($k:expr, $needle:expr, $trust_type:expr, $score:expr, $conf:expr) => {
            if $k.eq_ignore_ascii_case($needle) || $k.starts_with($needle) {
                env.trust(
                    $trust_type,
                    ScoreType::Env,
                    format!("Trusted user Env Key found: {}", $k),
                    $score, $conf,
                );
            }
        };
    }
    macro_rules! trust_val_contains {
        ($k:expr, $v:expr, $needle:expr, $trust_type:expr, $score:expr, $conf:expr) => {
            if $v.to_lowercase().contains(&$needle.to_lowercase()) {
                env.trust(
                    $trust_type,
                    ScoreType::Env,
                    format!("Highly trusted developer tool trace in [{}] : {}", $k, $needle),
                    $score, $conf,
                );
            }
        };
    }

    // 遍历所有环境变量进行规则校验
    for (k, v) in &all_vars {
        let k_str = k.as_str();
        let v_str = v.as_str();

        // ==========================================
        // 1. 风险与对抗特征 (Risk & Anti-Analysis)
        // ==========================================

        // === 沙箱专有环境变量 ===
        chk_key!(k_str, ss!("CUCKOO"),      sandbox, SandboxType::Cuckoo,     10, 0.95);
        chk_key!(k_str, ss!("ANYRUN"),      sandbox, SandboxType::Unknown,    10, 0.95); // Any.Run 沙箱
        chk_key!(k_str, ss!("CAPE"),        sandbox, SandboxType::CAPE,       10, 0.95);
        chk_key!(k_str, ss!("WEBSENSE"),    sandbox, SandboxType::Unknown,     9, 0.90);
        chk_key!(k_str, ss!("JOESANDBOX"),  sandbox, SandboxType::JoeSandbox, 10, 0.95);

        // === 虚拟化与模拟器 ===
        chk_key!(k_str, ss!("VBOX"),        virtual_machine, VirtualMachineType::VirtualBox, 9, 0.85);
        chk_key!(k_str, ss!("QEMU"),        emulator,        EmulatorType::QemuTCG,          9, 0.90);

        // === 容器与云环境 ===
        chk_key!(k_str, ss!("KUBERNETES"),  container, ContainerType::Kubernetes, 8, 0.90);
        chk_key!(k_str, ss!("DOCKER"),      container, ContainerType::Docker,     8, 0.85);
        chk_key!(k_str, ss!("WSL_DISTRO"),  container, ContainerType::Wsl,        8, 0.90); // Windows 里的 Linux 子系统

        // === Hook 与插桩分析工具 (Linux/Mac常见) ===
        chk_key!(k_str, ss!("LD_PRELOAD"),  software, SoftwareType::Analysis, 8, 0.70); // 常见于 API Hook
        chk_key!(k_str, ss!("PINTOOL"),     software, SoftwareType::Analysis, 9, 0.90); // Intel PIN 动态插桩
        chk_key!(k_str, ss!("FRIDA"),       software, SoftwareType::Debugger, 10, 0.95); // Frida 调试框架

        // === 高危用户身份特征 (通过 Value 匹配) ===
        // 沙箱常常使用特定的用户名
        chk_val!(k_str, v_str, ss!("USERNAME"), ss!("cuckoo"),   sandbox, SandboxType::Cuckoo,  10, 0.95);
        chk_val!(k_str, v_str, ss!("USERNAME"), ss!("sandbox"),  sandbox, SandboxType::Unknown,  9, 0.90);
        chk_val!(k_str, v_str, ss!("USERNAME"), ss!("malware"),  sandbox, SandboxType::Unknown,  9, 0.90);
        chk_val!(k_str, v_str, ss!("USERNAME"), ss!("virus"),    sandbox, SandboxType::Unknown,  9, 0.90);
        chk_val!(k_str, v_str, ss!("USERNAME"), ss!("test"),     sandbox, SandboxType::Unknown,  6, 0.60); // 测试机常见
        chk_val!(k_str, v_str, ss!("USERNAME"), ss!("vmware"),   virtual_machine, VirtualMachineType::VMware, 9, 0.85);

        // 计算机域名是沙箱特征
        chk_val!(k_str, v_str, ss!("USERDOMAIN"), ss!("sandbox"),sandbox, SandboxType::Unknown,  9, 0.90);


        // ==========================================
        // 2. 真实用户可信痕迹特征 (Trust)
        // ==========================================

        // === 云同步盘 (需要真实人类登录绑定) ===
        trust_key!(k_str, ss!("OneDrive"),           TrustType::CloudSync, 9, 0.95);
        trust_key!(k_str, ss!("OneDriveConsumer"),   TrustType::CloudSync, 9, 0.95);
        trust_key!(k_str, ss!("OneDriveCommercial"), TrustType::CloudSync, 9, 0.95);

        // === 开发者画像 (IDE与环境路径) ===
        trust_key!(k_str, ss!("GOPATH"),             TrustType::Development, 7, 0.85); // Go 开发
        trust_key!(k_str, ss!("CARGO_HOME"),         TrustType::Development, 7, 0.85); // Rust 开发
        trust_key!(k_str, ss!("ANDROID_HOME"),       TrustType::Development, 7, 0.85); // 安卓开发
        trust_key!(k_str, ss!("NVM_DIR"),            TrustType::Development, 7, 0.85); // Node.js (nvm)
        trust_key!(k_str, ss!("VIRTUAL_ENV"),        TrustType::Development, 7, 0.85); // Python venv
        chk_key!(k_str,   ss!("VSCODE_"),            trust, TrustType::Development, 8, 0.90); // 如 VSCODE_GIT_IPC_HANDLE
        chk_key!(k_str,   ss!("IDEA_INITIAL_"),      trust, TrustType::Development, 8, 0.90); // IntelliJ IDEA


        // === 物理终端交互验证 ===
        // 在 Windows 下，如果 SESSIONNAME 是 "Console"，意味着用户是坐在物理显示器前操作的。
        chk_val!(k_str, v_str, ss!("SESSIONNAME"), ss!("Console"), trust, TrustType::PhysicalDevices, 7, 0.75);

        // 检查显示器 DPI 缩放配置痕迹 (真实显示器特有)
        chk_key!(k_str,   ss!("QT_SCALE_FACTOR"),    trust, TrustType::PhysicalDevices, 6, 0.70);

        trust_val_contains!(k_str, v_str, ss!("ja-netfilter.jar"), TrustType::Development, 10, 0.98);
        trust_val_contains!(k_str, v_str, ss!("jrebel"), TrustType::Development, 8, 0.90);
        trust_val_contains!(k_str, v_str, ss!("xdebug"), TrustType::Development, 8, 0.90);

        trust_val_contains!(k_str, v_str, ss!("127.0.0.1:7890"), TrustType::Network, 9, 0.95);
        trust_val_contains!(k_str, v_str, ss!("127.0.0.1:10808"), TrustType::Network, 9, 0.95);
        chk_key!(k_str, ss!("ALL_PROXY"), trust, TrustType::Network, 8, 0.85);
        chk_key!(k_str, ss!("HTTP_PROXY"), trust, TrustType::Network, 6, 0.70);

        trust_val_contains!(k_str, v_str, ss!("registry.npmmirror.com"), TrustType::Development, 9, 0.95);
        trust_val_contains!(k_str, v_str, ss!("pypi.tuna.tsinghua.edu.cn"), TrustType::Development, 9, 0.95);
        trust_val_contains!(k_str, v_str, ss!("rsproxy.cn"), TrustType::Development, 9, 0.95);
        trust_val_contains!(k_str, v_str, ss!("goproxy.cn"), TrustType::Development, 9, 0.95);

        // Wayland 或 X11 显示环境
        chk_key!(k_str, ss!("WAYLAND_DISPLAY"), trust, TrustType::PhysicalDevices, 9, 0.95);
        chk_key!(k_str, ss!("DISPLAY"), trust, TrustType::PhysicalDevices, 6, 0.80);
        // 现代 Linux 桌面会话
        chk_key!(k_str, ss!("XDG_CURRENT_DESKTOP"), trust, TrustType::PhysicalDevices, 8, 0.90);
        chk_key!(k_str, ss!("DBUS_SESSION_BUS_ADDRESS"), trust, TrustType::SystemUptime, 7, 0.85);

        chk_key!(k_str, ss!("KITTY_WINDOW_ID"), trust, TrustType::InstalledSoftware, 9, 0.95);
        chk_key!(k_str, ss!("COLORTERM"), trust, TrustType::InstalledSoftware, 7, 0.80);
        // Oh-My-Zsh 痕迹
        trust_val_contains!(k_str, v_str, ss!(".oh-my-zsh"), TrustType::Development, 9, 0.95);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn sandbox_fs_test() {
        let mut env = Environment::new();

        check_env_vars(&mut env).await;

        println!("Risk Score: {:.2}%", env.final_risk_score());
        println!("{}", env.dump_report());
    }
}