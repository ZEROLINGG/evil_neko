use std::env as std_env;
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;
use crate::sandbox::*;
use crate::action;
use regex::Regex;

#[cfg(windows)]
fn get_systemapi_hostname() -> Option<HeapStr> {
    use std::os::windows::ffi::OsStringExt;

    #[allow(non_snake_case)]
    unsafe extern "system" {
        fn GetComputerNameW(lpBuffer: *mut u16, lpnSize: *mut u32) -> i32;
    }

    let mut buf = vec![0u16; 256];
    let mut size = buf.len() as u32;

    unsafe {
        if GetComputerNameW(buf.as_mut_ptr(), &mut size) != 0 && size > 0 {
            let string_val = std::ffi::OsString::from_wide(&buf[..size as usize])
                .to_string_lossy()
                .into_owned();
            return Some(HeapStr::from(string_val));
        }
    }
    None
}

#[cfg(unix)]
fn get_systemapi_hostname() -> Option<HeapStr> {
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
fn get_systemapi_hostname() -> Option<HeapStr> {
    None
}

// -----------------------------------------------------------

pub async fn check_hostname(env: Arc<Mutex<Environment>>) {
    let get_env_hostname = || -> Option<HeapStr> {
        #[cfg(windows)]
        { std_env::var(ss!("COMPUTERNAME")).ok().map(HeapStr::from) }
        #[cfg(unix)]
        { std_env::var(ss!("HOSTNAME")).ok().map(HeapStr::from) }
    };
    let env_host_opt = get_env_hostname();

    if env_host_opt != get_env_hostname() {
        env.lock().await.add(action!(
            AbnormalType::Inconsistent,
            ScoreType::OtherSystemApi,
            s_add!("Hostname env vars retrieved on two occasions were different."),
            7, 1.0
        ));
    }

    let sys_host_opt = get_systemapi_hostname();

    if sys_host_opt != get_systemapi_hostname() {
        env.lock().await.add(action!(
            AbnormalType::Inconsistent,
            ScoreType::OtherSystemApi,
            s_add!("System API hostnames obtained on two occasions were different."),
            8, 1.0
        ));
    }

    #[allow(unused)]
    let mut fs_host_opt: Option<HeapStr> = None;
    #[cfg(unix)]
    {
        if let Ok(contents) = std::fs::read_to_string(ss!("/etc/hostname")) {
            let trimmed = contents.trim();
            if !trimmed.is_empty() {
                fs_host_opt = Some(HeapStr::from(trimmed));
            }
        }
    }

    let env_host_ref = env_host_opt.as_deref().unwrap_or("");
    let sys_host_ref = sys_host_opt.as_deref().unwrap_or("");
    let fs_host_ref = fs_host_opt.as_deref().unwrap_or("");

    let env_valid = !env_host_ref.is_empty();
    let sys_valid = !sys_host_ref.is_empty();
    let fs_valid = !fs_host_ref.is_empty();

    // -- 交叉一致性校验 (Consistency Check) --
    if sys_valid && env_valid && !sys_host_ref.eq_ignore_ascii_case(env_host_ref) {
        let msg = s_add!("[Hostname] SysAPI vs Env mismatch: Sys=[", sys_host_ref, "] Env=[", env_host_ref, "]");
        env.lock().await.add(action!(
            AbnormalType::Inconsistent,
            ScoreType::OtherSystemApi,
            msg, 6, 0.9
        ));
    }

    if fs_valid && sys_valid && !fs_host_ref.eq_ignore_ascii_case(sys_host_ref) {
        let msg = s_add!("[Hostname] FS vs SysAPI mismatch: FS=[", fs_host_ref, "] Sys=[", sys_host_ref, "]");
        env.lock().await.add(action!(
            AbnormalType::Inconsistent,
            ScoreType::OtherSystemApi,
            msg, 8, 1.0
        ));
    }

    // 汇总需要校验的主机名
    let mut check_targets = Vec::new();
    if sys_valid { check_targets.push(sys_host_ref); }
    if env_valid { check_targets.push(env_host_ref); }
    if fs_valid { check_targets.push(fs_host_ref); }
    check_targets.sort();
    check_targets.dedup();

    static RE_DOCKER: LazyLock<Regex> = LazyLock::new(|| Regex::new(&ss!("^[a-f0-9]{12}$")).unwrap());
    static RE_K8S_POD: LazyLock<Regex> = LazyLock::new(|| Regex::new(&ss!(r"^.+-(?:[a-z0-9]+-)*[a-z0-9]+-[a-z0-9]{5}$")).unwrap());
    static RE_AWS_LINUX: LazyLock<Regex> = LazyLock::new(|| Regex::new(&ss!(r"^ip(?:-\d{1,3}){4}$")).unwrap());
    static RE_WIN_DESKTOP: LazyLock<Regex> = LazyLock::new(|| Regex::new(&ss!(r"^desktop-[a-z0-9]{7}$")).unwrap());
    static RE_WIN_SERVER: LazyLock<Regex> = LazyLock::new(|| Regex::new(&ss!(r"^win-[a-z0-9]{11,15}$")).unwrap());
    static RE_GITHUB_ACTIONS: LazyLock<Regex> = LazyLock::new(|| Regex::new(&ss!(r"^fv-az\d+-\d+$")).unwrap());
    static RE_ALIYUN_ECS: LazyLock<Regex> = LazyLock::new(|| Regex::new(&ss!(r"^iz[a-z0-9]{8,}z$")).unwrap());
    static RE_AZURE_APP: LazyLock<Regex> = LazyLock::new(|| Regex::new(&ss!(r"^rd[a-z0-9]{8,}$")).unwrap());

    for target_host in check_targets {
        let host_lower = target_host.to_ascii_lowercase();

        //  Docker 默认主机名
        if RE_DOCKER.is_match(&host_lower) {
            env.lock().await.add(action!(
                ContainerType::Docker, ScoreType::OtherSystemApi,
                s_add!("Docker default hostname pattern detected: ", target_host),
                5, 0.8
            ));
        }

        //  Kubernetes 默认主机名
        if RE_K8S_POD.is_match(&host_lower) {
            env.lock().await.add(action!(
                ContainerType::Kubernetes, ScoreType::OtherSystemApi,
                s_add!("Kubernetes pod hostname pattern detected: ", target_host),
                3, 0.8
            ));
        }

        //  AWS 环境
        if RE_AWS_LINUX.is_match(&host_lower) {
            env.lock().await.add(action!(
                TrustType::Development, ScoreType::OtherSystemApi,
                s_add!("AWS EC2 Linux internal hostname pattern detected: ", target_host),
                8, 0.85
            ));
        }
        if host_lower.starts_with(&ss!("ec2amaz-")) {
            env.lock().await.add(action!(
                TrustType::Development, ScoreType::OtherSystemApi,
                s_add!("AWS EC2 Windows hostname pattern detected: ", target_host),
                8, 0.85
            ));
        }

        if RE_WIN_DESKTOP.is_match(&host_lower) {
            #[cfg(unix)]
            env.lock().await.add(action!(
                ContainerType::Wsl, ScoreType::OtherSystemApi,
                s_add!("WSL detected based on hostname: ", target_host),
                6, 0.7
            ));

            #[cfg(windows)]
            env.lock().await.add(action!(
                VirtualMachineType::Unknown, ScoreType::OtherSystemApi,
                s_add!("Default Windows 10/11 hostname detected: ", target_host),
                2, 0.2
            ));
        }

        if RE_WIN_SERVER.is_match(&host_lower) {
            env.lock().await.add(action!(
                TrustType::Development, ScoreType::OtherSystemApi,
                s_add!("Default Windows Server hostname detected: ", target_host),
                4, 0.5
            ));
        }

        if RE_GITHUB_ACTIONS.is_match(&host_lower) || host_lower.starts_with(&ss!("mac-16")) {
            env.lock().await.add(action!(
                TrustType::Development, ScoreType::OtherSystemApi,
                s_add!("GitHub Actions CI/CD runner detected: ", target_host),
                8, 0.9
            ));
        }
        if host_lower.starts_with(&ss!("runner-")) && host_lower.contains(&ss!("-project-")) {
            env.lock().await.add(action!(
                TrustType::Development, ScoreType::OtherSystemApi,
                s_add!("GitLab CI runner detected: ", target_host),
                8, 0.9
            ));
        }

        if RE_ALIYUN_ECS.is_match(&host_lower) {
            env.lock().await.add(action!(
                TrustType::Development, ScoreType::OtherSystemApi,
                s_add!("Alibaba Cloud ECS hostname pattern detected: ", target_host),
                5, 0.6
            ));
        }

        if RE_AZURE_APP.is_match(&host_lower) {
            env.lock().await.add(action!(
                TrustType::Development, ScoreType::OtherSystemApi,
                s_add!("Azure App Service hostname pattern detected: ", target_host),
                6, 0.7
            ));
        }

        if host_lower == ss!("raspberrypi") {
            env.lock().await.add(action!(
                TrustType::Development, ScoreType::OtherSystemApi,
                s_add!("Raspberry Pi default hostname detected: ", target_host),
                2, 0.2
            ));
        }

        // --- 黑名单匹配与记分逻辑 ---
        let mut is_blacklisted = false;
        let mut score = 0;
        let mut confidence = 0.0;
        let mut matched_name: HeapStr = HeapStr::default();
        let mut sandbox_type = SandboxType::Unknown;

        macro_rules! check_black {
            ($name:expr, $sc:expr, $r:expr, $stype:expr) => {
                if !is_blacklisted && host_lower.contains(&ss!($name)) {
                    is_blacklisted = true;
                    score = $sc;
                    confidence = $r;
                    matched_name = s!($name).into();
                    sandbox_type = $stype;
                }
            };
        }

        check_black!("sandbox", 8, 0.8, SandboxType::Unknown);
        check_black!("cuckoo", 9, 0.9, SandboxType::Cuckoo);
        check_black!("tequilaboomboom", 10, 1.0, SandboxType::Cuckoo); // Cuckoo
        check_black!("joe", 7, 0.7, SandboxType::JoeSandbox);
        check_black!("vxstream", 9, 0.9, SandboxType::Unknown);        // Hybrid Analysis
        check_black!("vmray", 9, 0.9, SandboxType::Unknown);           // VMRay
        check_black!("fireeye", 9, 0.9, SandboxType::Unknown);         // FireEye
        check_black!("mandiant", 9, 0.9, SandboxType::Unknown);
        check_black!("anyrun", 9, 0.9, SandboxType::Unknown);          // ANY.RUN
        check_black!("fortinet", 7, 0.7, SandboxType::Unknown);

        check_black!("vbox", 6, 0.7, SandboxType::Unknown);            // VirtualBox
        check_black!("vmware", 6, 0.7, SandboxType::Unknown);
        check_black!("qemu", 7, 0.8, SandboxType::Unknown);
        check_black!("parallels", 6, 0.6, SandboxType::Unknown);

        check_black!("mcafee", 8, 0.8, SandboxType::Unknown);
        check_black!("kaspersky", 8, 0.8, SandboxType::Unknown);

        check_black!("maltest", 8, 0.8, SandboxType::Unknown);
        check_black!("malware", 10, 1.0, SandboxType::Unknown);
        check_black!("virus", 10, 1.0, SandboxType::Unknown);
        check_black!("analysis", 8, 0.8, SandboxType::Unknown);
        check_black!("klone", 7, 0.8, SandboxType::Unknown);
        check_black!("kloned", 7, 0.8, SandboxType::Unknown);

        check_black!("user-pc", 3, 0.5, SandboxType::Unknown);
        check_black!("admin-pc", 3, 0.5, SandboxType::Unknown);
        check_black!("test-pc", 5, 0.6, SandboxType::Unknown);
        check_black!("john-pc", 5, 0.6, SandboxType::Unknown);
        check_black!("desktop-system", 6, 0.6, SandboxType::Unknown);
        check_black!("win7-", 3, 0.4, SandboxType::Unknown);

        if !is_blacklisted && host_lower.contains(&ss!("kali")) {
            env.lock().await.add(action!(
                SoftwareType::Analysis,
                ScoreType::OtherSystemApi,
                s!("hostname is kali"),
                7,
                0.8
            ));
        }

        if is_blacklisted {
            env.lock().await.add(action!(
                sandbox_type,
                ScoreType::OtherSystemApi,
                s_add!("Suspicious/Sandbox hostname detected (Matched: ", matched_name.as_str(), ") -> ", target_host),
                score,
                confidence
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hostname_check() {
        let env = Environment::new();
        check_hostname(env.clone()).await;
        println!("{}", env.lock().await.dump_report());
    }
}