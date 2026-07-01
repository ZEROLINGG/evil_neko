// lib/src/sandbox/env.rs
#![allow(unused)]
#[libpm::rt(s1)]

use std::env as std_env;
use libpm::*;
use crate::sandbox::*;
use crate::{action};


macro_rules! a {
    ($action_type:expr, $score:expr, $confidence:expr) => {{
        $action_type.into_action($crate::sandbox::ScoreType::Env, "".into(), $score, $confidence)
    }};
    ($action_type:expr, $msg:expr, $score:expr, $confidence:expr) => {{
        $action_type.into_action($crate::sandbox::ScoreType::Env, $msg.into(), $score, $confidence)
    }};
}


fn chk_key(actions: &mut Vec<ScoreAction>, kv: &(String, String), match_key: &str, mut action: ScoreAction) {
    if kv.0 == match_key {
        action.set_msg(s_add!("Env Key found: ", kv.0));
        actions.push(action);
    }
}

fn chk_key_prefix(actions: &mut Vec<ScoreAction>, kv: &(String, String), prefix: &str, mut action: ScoreAction) {
    if kv.0.starts_with(prefix) {
        action.set_msg(s_add!("Env Key prefix found: ", kv.0));
        actions.push(action);
    }
}

fn chk_val(actions: &mut Vec<ScoreAction>, kv: &(String, String), contains: &str, mut action: ScoreAction) {
    if kv.1.contains(&contains) {
        action.set_msg(s_fmt!("[{}] contains: {}", kv.0, contains));
        actions.push(action);
    }
}
fn chk_val_msg(actions: &mut Vec<ScoreAction>, kv: &(String, String), contains: &str, msg: &str, mut action: ScoreAction) {
    if kv.1.contains(&contains) {
        action.set_msg(msg.to_string());
        actions.push(action);
    }
}

fn chk_kv(actions: &mut Vec<ScoreAction>, kv: &(String, String), match_key: &str, contains: &[&str], mut action: ScoreAction) {
    if kv.0 != match_key { return; }
    for contain in contains {
        if kv.1.contains(contain) {
            action.set_msg(s_add!("[", kv.0, "] contains: ", contain));
            actions.push(action);
            break; // 命中一个即转移所有权并跳出
        }
    }
}

fn chk_trap(actions: &mut Vec<ScoreAction>, kv: &(String, String), prefix: &str, msg_prefix: &str, mut action: ScoreAction) {
    if kv.0.starts_with(prefix) {
        action.set_msg(s_add!(msg_prefix, ": ", kv.0));
        actions.push(action);
    }
}


// ==========================================
// 业务逻辑函数
// ==========================================

pub async fn check_env(env: Arc<Mutex<Environment>>) {
    let all_vars: Vec<(String, String)> = std_env::vars().map(|(k, v)| (k.to_uppercase(), v.to_lowercase())).collect();
    let env_count = all_vars.len();
    let mut actions: Vec<ScoreAction> = Vec::new();

    if env_count < 15 {
        actions.push(action!(
            SandboxType::Unknown,
            ScoreType::Env,
            s_add!("Unusually low environment variables: ", env_count),
            5, 0.75
        ));
    } else if env_count > 45 {
        actions.push(action!(
            TrustType::UserTraces,
            ScoreType::Env,
            s_add!("Sufficient environmental variables: ", env_count),
            6, 0.80
        ));
    }

    for kv in &all_vars {
        // ==========================================
        // Risk
        // ==========================================


        // === 沙箱专有环境变量 ===
        chk_key(&mut actions, kv, ss!("CUCKOO"), a!(SandboxType::Cuckoo, 10, 0.95));
        chk_key(&mut actions, kv, ss!("ANYRUN"), a!(SandboxType::Unknown, 10, 0.95));
        chk_key(&mut actions, kv, ss!("CAPE"), a!(SandboxType::CAPE, 10, 0.95));
        chk_key(&mut actions, kv, ss!("WEBSENSE"), a!(SandboxType::Unknown, 9, 0.90));
        chk_key(&mut actions, kv, ss!("JOESANDBOX"), a!(SandboxType::JoeSandbox, 10, 0.95));

        // === 虚拟化与模拟器 ===
        chk_key(&mut actions, kv, ss!("VBOX"), a!(VirtualMachineType::VirtualBox, 9, 0.85));
        chk_key(&mut actions, kv, ss!("QEMU"), a!(EmulatorType::QemuTCG, 9, 0.90));

        // === 容器与云环境 ===
        chk_key(&mut actions, kv, ss!("KUBERNETES"), a!(ContainerType::Kubernetes, 8, 0.90));
        chk_key(&mut actions, kv, ss!("DOCKER"), a!(ContainerType::Docker, 8, 0.85));
        chk_key_prefix(&mut actions, kv, ss!("WSL_DISTRO"), a!(ContainerType::Wsl, 8, 0.90));

        // === Hook 与插桩分析工具 ===
        chk_key(&mut actions, kv, ss!("LD_PRELOAD"), a!(SoftwareType::Analysis, 8, 0.70));
        chk_key(&mut actions, kv, ss!("PINTOOL"), a!(SoftwareType::Analysis, 9, 0.90));
        chk_key(&mut actions, kv, ss!("FRIDA"), a!(SoftwareType::Debugger, 10, 0.95));

        // === 高危用户身份特征 ===
        chk_kv(&mut actions, kv, ss!("USERNAME"), &[ss!("Administrator")], a!(SandboxType::Unknown, 6, 0.75));
        chk_kv(&mut actions, kv, ss!("USERNAME"), &[ss!("cuckoo")], a!(SandboxType::Cuckoo, 10, 0.95));
        chk_kv(&mut actions, kv, ss!("USERNAME"), &[ss!("sandbox"), ss!("malware"), ss!("virus")], a!(SandboxType::Unknown, 9, 0.90));
        chk_kv(&mut actions, kv, ss!("USERNAME"), &[ss!("test")], a!(SandboxType::Unknown, 6, 0.60));
        chk_kv(&mut actions, kv, ss!("USERNAME"), &[ss!("vmware")], a!(VirtualMachineType::VMware, 9, 0.85));
        chk_kv(&mut actions, kv, ss!("USERDOMAIN"), &[ss!("sandbox")], a!(SandboxType::Unknown, 9, 0.90));

        // ==========================================
        // Trust
        // ==========================================

        // === 开发者画像 ===
        chk_key(&mut actions, kv, ss!("GOPATH"), a!(TrustType::Development, 7, 0.85));
        chk_key(&mut actions, kv, ss!("CARGO_HOME"), a!(TrustType::Development, 7, 0.85));
        chk_key(&mut actions, kv, ss!("ANDROID_HOME"), a!(TrustType::Development, 7, 0.85));
        chk_key(&mut actions, kv, ss!("NVM_DIR"), a!(TrustType::Development, 7, 0.85));
        chk_key(&mut actions, kv, ss!("VIRTUAL_ENV"), a!(TrustType::Development, 7, 0.85));
        chk_key_prefix(&mut actions, kv, ss!("VSCODE_"), a!(TrustType::Development, 8, 0.90));
        chk_key_prefix(&mut actions, kv, ss!("IDEA_INITIAL_"), a!(TrustType::Development, 8, 0.90));

        // === 物理终端交互验证 ===
        chk_key(&mut actions, kv, ss!("QT_SCALE_FACTOR"), a!(TrustType::PhysicalDevices, 6, 0.70));

        // === 开发工具 Value 特征 ===
        chk_val(&mut actions, kv, ss!("ja-netfilter.jar"), a!(TrustType::Development, 10, 0.98));
        chk_val(&mut actions, kv, ss!("jrebel"), a!(TrustType::Development, 8, 0.90));
        chk_val(&mut actions, kv, ss!("xdebug"), a!(TrustType::Development, 8, 0.90));

        // === 代理与镜像源 ===
        chk_val(&mut actions, kv, ss!("127.0.0.1:7890"), a!(TrustType::Network, 9, 0.95));
        chk_val(&mut actions, kv, ss!("127.0.0.1:10808"), a!(TrustType::Network, 9, 0.95));
        chk_key(&mut actions, kv, ss!("ALL_PROXY"), a!(TrustType::Network, 8, 0.85));
        chk_key(&mut actions, kv, ss!("HTTP_PROXY"), a!(TrustType::Network, 6, 0.70));
        chk_val(&mut actions, kv, ss!("registry.npmmirror.com"), a!(TrustType::Development, 9, 0.95));
        chk_val(&mut actions, kv, ss!("pypi.tuna.tsinghua.edu.cn"), a!(TrustType::Development, 9, 0.95));
        chk_val(&mut actions, kv, ss!("rsproxy.cn"), a!(TrustType::Development, 9, 0.95));
        chk_val(&mut actions, kv, ss!("goproxy.cn"), a!(TrustType::Development, 9, 0.95));

        #[cfg(unix)]
        {
            // === Wayland / X11 桌面环境 ===
            chk_key(&mut actions, kv, ss!("DISPLAY"), a!(TrustType::PhysicalDevices, 4, 0.80));
            chk_key(&mut actions, kv, ss!("XDG_CURRENT_DESKTOP"), a!(TrustType::PhysicalDevices, 4, 0.90));
            chk_key_prefix(&mut actions, kv, ss!("DBUS_"), a!(TrustType::PhysicalDevices, 7, 0.85));

            // === 终端与 Shell ===
            chk_key_prefix(&mut actions, kv, ss!("KITTY_"), a!(TrustType::InstalledSoftware, 9, 0.95));
            chk_key(&mut actions, kv, ss!("COLORTERM"), a!(TrustType::InstalledSoftware, 7, 0.80));
            chk_val(&mut actions, kv, ss!(".oh-my-zsh"), a!(TrustType::Development, 9, 0.95));
        }
    }

    if !actions.is_empty() {
        env.lock().await.add_all(actions);
    }
}


pub async fn check_env_trap(env: Arc<Mutex<Environment>>) {
    let all_vars: Vec<(String, String)> = std_env::vars().map(|(k, v)| (k.to_uppercase(), v.to_lowercase())).collect();
    let mut actions: Vec<ScoreAction> = Vec::new();


    #[cfg(windows)]
    {
        for kv in &all_vars {

            // 1. 直接的 Wine 特征 (Wine 自身暴露)
            let wine_msg = s!("Wine env detected");
            chk_trap(&mut actions, kv, ss!("WINEPREFIX"), &wine_msg, a!(EmulatorType::Wine, 10, 1.0));
            chk_trap(&mut actions, kv, ss!("WINEDEBUG"), &wine_msg, a!(EmulatorType::Wine, 10, 1.0));
            chk_trap(&mut actions, kv, ss!("WINEUSERNAME"), &wine_msg, a!(EmulatorType::Wine, 10, 1.0));

            // 2. Linux 桌面与显示协议 (X11 / Wayland)
            let linux_env_msg = s!("Unix/Linux env detected into Windows");
            chk_trap(&mut actions, kv, ss!("XDG_"), &linux_env_msg, a!(EmulatorType::Unknown, 10, 0.98));
            chk_trap(&mut actions, kv, ss!("WAYLAND_DISPLAY"), &linux_env_msg, a!(EmulatorType::Unknown, 10, 0.98));
            chk_trap(&mut actions, kv, ss!("DISPLAY"), &linux_env_msg, a!(EmulatorType::Unknown, 9, 0.90));
            chk_trap(&mut actions, kv, ss!("DBUS_"), &linux_env_msg, a!(EmulatorType::Unknown, 9, 0.95));

            // 3. Linux 终端与 Shell 泄漏
            chk_trap(&mut actions, kv, ss!("ZSH"), &linux_env_msg, a!(EmulatorType::Unknown, 9, 0.95));
            chk_trap(&mut actions, kv, ss!("COLORTERM"), &linux_env_msg, a!(EmulatorType::Unknown, 9, 0.90));
            chk_trap(&mut actions, kv, ss!("SSH_AUTH_SOCK"), &linux_env_msg, a!(EmulatorType::Unknown, 9, 0.95));
            chk_trap(&mut actions, kv, ss!("KITTY_"), &linux_env_msg, a!(EmulatorType::Unknown, 9, 0.95));


            chk_val_msg(&mut actions, kv, ss!("/usr/bin"), s_add!(&linux_env_msg, ": /usr/bin").as_str(), a!(EmulatorType::Unknown, 3, 0.95));
            chk_val_msg(&mut actions, kv, ss!("/home"), s_add!(&linux_env_msg, ": /home").as_str(), a!(EmulatorType::Unknown, 4, 0.8));

        }
    }

    #[cfg(unix)]
    {
        for kv in &all_vars {

            let win_env_msg = s!("Windows env detected into Unix/Linux");
            chk_trap(&mut actions, kv, ss!("USERPROFILE"), &win_env_msg, a!(EmulatorType::Unknown, 10, 0.95));
            chk_trap(&mut actions, kv, ss!("APPDATA"), &win_env_msg, a!(EmulatorType::Unknown, 10, 0.95));
            chk_trap(&mut actions, kv, ss!("LOCALAPPDATA"), &win_env_msg, a!(EmulatorType::Unknown, 10, 0.95));
            chk_trap(&mut actions, kv, ss!("PROGRAMFILES"), &win_env_msg, a!(EmulatorType::Unknown, 10, 0.95));
            chk_trap(&mut actions, kv, ss!("COMPUTERNAME"), &win_env_msg, a!(EmulatorType::Unknown, 9, 0.90));
            chk_trap(&mut actions, kv, ss!("OS"), ss!("CRITICAL TRAP: OS=Windows_NT leaked into Linux"), a!(EmulatorType::Unknown, 7, 0.80));

            chk_trap(&mut actions, kv, ss!("QEMU_"), ss!("CRITICAL TRAP: QEMU User-Mode Emulation detected"), a!(EmulatorType::QemuTCG, 10, 1.0));
            chk_trap(&mut actions, kv, ss!("AFL_"), ss!("CRITICAL TRAP: Fuzzer (AFL) execution detected"), a!(EmulatorType::Unknown, 10, 1.0));
            chk_trap(&mut actions, kv, ss!("TERMUX_VERSION"), ss!("CRITICAL TRAP: Running in Android Termux"), a!(EmulatorType::Unknown, 8, 0.90));
            chk_trap(&mut actions, kv, ss!("PROOT_TMP_DIR"), ss!("CRITICAL TRAP: Running in PRoot environment"), a!(EmulatorType::Unknown, 8, 0.90));

            chk_val_msg(&mut actions, kv, ss!("C:\\Users\\"), s_add!(win_env_msg, ": C:\\Users\\").as_str(), a!(EmulatorType::Unknown, 3, 0.95));
        }
    }

    if !actions.is_empty() {
        env.lock().await.add_all(actions);
    }

}

#[cfg(test)]
mod tests {
    use libpm::*;
    use super::*;
    #[tokio::test]
    async fn sandbox_fs_test() {
        let env = Environment::new();
        check_env(env.clone()).await;
        check_env_trap(env.clone()).await;
        sprint!(env.lock().await.dump_report());
    }
}

