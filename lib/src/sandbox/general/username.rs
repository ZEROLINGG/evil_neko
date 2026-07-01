use std::env as std_env;
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;
use crate::sandbox::*;
use crate::{action};
use regex::Regex;

static USERNAME_PATTERNS: LazyLock<UsernamePatterns> = LazyLock::new(UsernamePatterns::new);

struct UsernamePatterns {
    administrator: Regex,
    test: Regex,
    wdagutility: Regex,
    vmware: Regex,
    virtualbox: Regex,
    sandbox: Regex,
    cuckoo: Regex,
    malware: Regex,
    virus: Regex,
    joesandbox: Regex,
    threat: Regex,
}

impl UsernamePatterns {
    fn new() -> Self {
        Self {
            administrator: Regex::new(ss!(r"(?i)^administrator$")).unwrap(),
            test: Regex::new(ss!(r"(?i)^test$")).unwrap(),
            wdagutility: Regex::new(ss!(r"(?i)wdagutility")).unwrap(),
            vmware: Regex::new(ss!(r"(?i)vmware")).unwrap(),
            virtualbox: Regex::new(ss!(r"(?i)(vbox|virtualbox)")).unwrap(),
            sandbox: Regex::new(ss!(r"(?i)sandbox")).unwrap(),
            cuckoo: Regex::new(ss!(r"(?i)cuckoo")).unwrap(),
            malware: Regex::new(ss!(r"(?i)malware")).unwrap(),
            virus: Regex::new(ss!(r"(?i)virus")).unwrap(),
            joesandbox: Regex::new(ss!(r"(?i)^joe$")).unwrap(),
            threat: Regex::new(ss!(r"(?i)threat")).unwrap(),
        }
    }
}

fn is_running_on_desktop() -> bool {
    if let Ok(exe_path) = std_env::current_exe() {
        let path_str = exe_path.to_string_lossy().to_lowercase();

        #[cfg(windows)]
        {
            return path_str.contains(r"\desktop\")
                || path_str.contains(r"\users\public\desktop\");
        }

        #[cfg(unix)]
        {
            if cfg!(target_os = "macos") {
                return path_str.contains("/desktop/") || path_str.contains("/users/shared/desktop/");
            } else {
                // Linux
                return path_str.contains("/desktop/")
                    || path_str.contains("/桌面/"); // 支持中文桌面
            }
        }

        #[cfg(not(any(windows, unix)))]
        {
            return path_str.contains("desktop");
        }
    }
    false
}

#[cfg(windows)]
fn get_systemapi_user() -> Option<HeapStr> {
    use std::os::windows::ffi::OsStringExt;

    #[allow(non_snake_case)]
    unsafe extern "system" {
        fn GetUserNameW(lpBuffer: *mut u16, pcbBuffer: *mut u32) -> i32;
    }

    let mut buf = vec![0u16; 512];
    let mut size = buf.len() as u32;

    unsafe {
        if GetUserNameW(buf.as_mut_ptr(), &mut size) != 0 && size > 0 {
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
fn get_systemapi_user() -> Option<HeapStr> {
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
fn get_systemapi_user() -> Option<HeapStr> {
    None
}

// -----------------------------------------------------------

/// 从可执行文件路径中提取用户名
fn extract_user_from_path() -> Option<HeapStr> {
    let exe_path = std_env::current_exe().ok()?;
    let path_str = exe_path.to_string_lossy();

    #[cfg(windows)]
    {
        // Windows路径格式: C:\Users\username\...
        // 使用正则表达式提取
        let re = regex::Regex::new(r"(?i)^[A-Z]:\\[Uu][Ss][Ee][Rr][Ss]\\([^\\]+)").ok()?;
        if let Some(caps) = re.captures(&path_str) {
            if let Some(username) = caps.get(1) {
                let user = username.as_str();
                // 排除系统保留用户
                if !user.eq_ignore_ascii_case("public")
                    && !user.eq_ignore_ascii_case("default")
                    && !user.eq_ignore_ascii_case("all users") {
                    return Some(HeapStr::from(user));
                }
            }
        }
    }

    #[cfg(unix)]
    {
        if cfg!(target_os = "macos") {
            // macOS路径: /Users/username/... 或 /var/root/...
            let re = regex::Regex::new(r"^/[Uu][Ss][Ee][Rr][Ss]/([^/]+)").ok()?;
            if let Some(caps) = re.captures(&path_str) {
                if let Some(username) = caps.get(1) {
                    let user = username.as_str();
                    if !user.eq_ignore_ascii_case("shared") {
                        return Some(HeapStr::from(user));
                    }
                }
            } else if path_str.starts_with("/var/root/") || path_str.starts_with("/root/") {
                return Some(s!("root").into());
            }
        } else {
            // Linux路径: /home/username/... 或 /root/...
            let re = regex::Regex::new(r"^/home/([^/]+)").ok()?;
            if let Some(caps) = re.captures(&path_str) {
                if let Some(username) = caps.get(1) {
                    return Some(HeapStr::from(username.as_str()));
                }
            } else if path_str.starts_with("/root/") {
                return Some(s!("root").into());
            }
        }
    }

    None
}

pub async fn check_username(env: Arc<Mutex<Environment>>) {
    if is_running_on_desktop() {
        env.lock().await.add(action!(
            SandboxType::Unknown,
            ScoreType::OtherSystemApi,
            s_add!("Executable is running from Desktop folder (common sandbox behavior)"),
            1,
            0.75
        ));
    }

    // 获取正则表达式模式 (直接借用 LazyLock)
    let patterns = &*USERNAME_PATTERNS;

    // 1. 获取环境变量中的用户名
    let get_env_user = || -> Option<HeapStr> {
        #[cfg(windows)]
        {
            std_env::var(ss!("USERNAME"))
                .or_else(|_| std_env::var(ss!("USER")))
                .ok()
                .map(HeapStr::from)
        }
        #[cfg(unix)]
        {
            std_env::var(ss!("USER"))
                .or_else(|_| std_env::var(ss!("LOGNAME")))
                .ok()
                .map(HeapStr::from)
        }
        #[cfg(not(any(windows, unix)))]
        {
            None
        }
    };

    let env_user_opt = get_env_user();

    // Anti-Hook 检测 - 环境变量
    if env_user_opt != get_env_user() {
        env.lock().await.add(action!(
            AbnormalType::Inconsistent,
            ScoreType::OtherSystemApi,
            s_add!("Environment variable USERNAME changed between calls (possible hook detected)"),
            7,
            1.0
        ));
    }

    if env_user_opt.is_none() {
        env.lock().await.add(action!(
            AbnormalType::Unknown,
            ScoreType::OtherSystemApi,
            s_add!("Username environment variable missing"),
            7,
            1.0
        ));
    }

    // 2. 获取系统API返回的用户名
    let systemapi_user_opt = get_systemapi_user();

    // Anti-Hook 检测 - 系统API
    if systemapi_user_opt != get_systemapi_user() {
        env.lock().await.add(action!(
            AbnormalType::Inconsistent,
            ScoreType::OtherSystemApi,
            s_add!("System API username changed between calls (possible hook detected)"),
            7,
            1.0
        ));
    }

    if systemapi_user_opt.is_none() {
        env.lock().await.add(action!(
            AbnormalType::SystemApi,
            ScoreType::OtherSystemApi,
            s_add!("Failed to obtain username via System API"),
            7,
            1.0
        ));
    }

    // 3. 从文件系统路径提取用户名（修复后）
    let fs_user_opt = match std_env::current_exe() {
        Ok(_) => extract_user_from_path(),
        Err(_) => {
            env.lock().await.add(action!(
                AbnormalType::SystemApi,
                ScoreType::File,
                s!("Failed to obtain executable file path"),
                9,
                1.0
            ));
            None
        }
    };

    // 获取引用
    let env_user_ref = env_user_opt.as_deref().unwrap_or("");
    let sys_user_ref = systemapi_user_opt.as_deref().unwrap_or("");
    let fs_user_ref = fs_user_opt.as_deref().unwrap_or("");

    let env_valid = !env_user_ref.is_empty();
    let sys_valid = !sys_user_ref.is_empty();
    let fs_valid = !fs_user_ref.is_empty();

    // 判断是否是系统账户
    let is_system_account = |user: &str| -> bool {
        user.eq_ignore_ascii_case(ss!("system"))
            || user.eq_ignore_ascii_case(ss!("local service"))
            || user.eq_ignore_ascii_case(ss!("network service"))
            || user.eq_ignore_ascii_case(ss!("defaultaccount"))
            || user.eq_ignore_ascii_case(ss!("root"))
            || user.eq_ignore_ascii_case(ss!("daemon"))
            || user.eq_ignore_ascii_case(ss!("sys"))
    };

    let is_running_as_system = sys_valid && is_system_account(sys_user_ref);

    // 4. 交叉验证（仅在非系统账户时）
    if !is_running_as_system {
        // 系统API vs 环境变量
        if sys_valid && env_valid && !sys_user_ref.eq_ignore_ascii_case(env_user_ref) {
            let msg = s_add!(
                "[username] SysAPI vs Env mismatch: Sys=[",
                sys_user_ref,
                "] Env=[",
                env_user_ref,
                "]"
            );
            env.lock().await.add(action!(
                AbnormalType::Inconsistent,
                ScoreType::OtherSystemApi,
                msg,
                5,
                0.8
            ));
        }

        // 文件系统 vs 系统API
        if fs_valid && sys_valid && !fs_user_ref.eq_ignore_ascii_case(sys_user_ref) {
            let msg = s_add!(
                "[username] FS vs SysAPI mismatch: FS=[",
                fs_user_ref,
                "] Sys=[",
                sys_user_ref,
                "]"
            );
            env.lock().await.add(action!(
                AbnormalType::Inconsistent,
                ScoreType::OtherSystemApi,
                msg,
                3,
                0.6
            ));
        }

        // 文件系统 vs 环境变量
        if fs_valid && env_valid && !fs_user_ref.eq_ignore_ascii_case(env_user_ref) {
            let msg = s_add!(
                "[username] FS vs Env mismatch: FS=[",
                fs_user_ref,
                "] Env=[",
                env_user_ref,
                "]"
            );
            env.lock().await.add(action!(
                AbnormalType::Inconsistent,
                ScoreType::OtherSystemApi,
                msg,
                3,
                0.6
            ));
        }
    }

    // 5. 收集所有需要检查的用户名（去重）
    let mut check_targets = Vec::new();
    if sys_valid {
        check_targets.push(sys_user_ref);
    }
    if env_valid {
        check_targets.push(env_user_ref);
    }
    if fs_valid {
        check_targets.push(fs_user_ref);
    }

    check_targets.sort();
    check_targets.dedup();

    // 6. 使用正则表达式模式匹配黑名单
    for target_user in check_targets {
        check_blacklist_pattern(target_user, patterns, &env).await;
    }
}

/// 使用正则表达式检查黑名单模式
async fn check_blacklist_pattern(
    username: &str,
    patterns: &UsernamePatterns,
    env: &Arc<Mutex<Environment>>
) {
    // 定义检查宏
    macro_rules! check_pattern {
        ($pattern:expr, $name:expr, $score:expr, $confidence:expr) => {
            if $pattern.is_match(username) {
                env.lock().await.add(action!(
                    SandboxType::Unknown,
                    ScoreType::OtherSystemApi,
                    s_add!("Suspicious username detected: ", username, " (matched pattern: ", $name, ")"),
                    $score,
                    $confidence
                ));
                return;
            }
        };
    }

    // 按风险等级从高到低检查
    check_pattern!(patterns.vmware, "vmware", 9, 0.9);
    check_pattern!(patterns.virtualbox, "virtualbox/vbox", 9, 0.9);
    check_pattern!(patterns.sandbox, "sandbox", 9, 0.9);
    check_pattern!(patterns.cuckoo, "cuckoo", 9, 0.9);
    check_pattern!(patterns.malware, "malware", 8, 0.8);
    check_pattern!(patterns.wdagutility, "wdagutility", 8, 0.8);
    check_pattern!(patterns.threat, "threat", 7, 0.8);
    check_pattern!(patterns.virus, "virus", 7, 0.8);
    check_pattern!(patterns.joesandbox, "joe", 6, 0.8);
    check_pattern!(patterns.test, "test", 3, 0.5);
    check_pattern!(patterns.administrator, "administrator", 2, 0.4);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_username_extraction() {
        // 测试路径提取
        if let Some(user) = extract_user_from_path() {
            println!("Extracted username from path: {}", user.as_str());
        } else {
            println!("Failed to extract username from path");
        }
    }

    #[tokio::test]
    async fn test_systemapi_user() {
        println!("SystemAPI user: {:?}", get_systemapi_user());
    }

    #[tokio::test]
    async fn sandbox_fs_test() {
        let env = Environment::new();
        check_username(env.clone()).await;
        sprint!(env.lock().await.dump_report());
    }

    #[tokio::test]
    async fn test_patterns() {
        // 获取正则表达式模式 (直接借用 LazyLock)
        let patterns = &*USERNAME_PATTERNS;

        let test_cases = vec![
            ("vboxuser", true),
            ("VBOXUSER", true),
            ("VBoxUser", true),
            ("vmware-user", true),
            ("sandbox123", true),
            ("normaluser", false),
            ("john", false),
            ("administrator", true),
            ("Administrator", true),
            ("test", true),
            ("testing", false),
        ];

        for (username, should_match) in test_cases {
            let matched = patterns.virtualbox.is_match(username)
                || patterns.vmware.is_match(username)
                || patterns.sandbox.is_match(username)
                || patterns.administrator.is_match(username)
                || patterns.test.is_match(username);

            assert_eq!(
                matched,
                should_match,
                "Username '{}' match result mismatch",
                username
            );
        }

        println!("All pattern tests passed!");
    }
}