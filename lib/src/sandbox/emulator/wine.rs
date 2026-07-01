#![cfg(windows)]
use crate::action;
use crate::sandbox::*;
use regex::Regex;
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;




pub async fn check_wine(env: Arc<Mutex<Environment>>) {
    use crate::utils::{get_parent_process, get_running_processes};
    use crate::sandbox::utils::fs::{__file_diff, __has_file, SYSTEM32};
    use crate::sandbox::utils::env::{__env_has_key, __env_has_kv};
    use crate::utils::win::resolve;


    let processes = get_running_processes();
    let parent = get_parent_process();

    let mut env_lock = env.lock().await;

    if let Some(p) = parent {
        static WINE_PARENT: LazyLock<Regex, fn() -> Regex> = LazyLock::new(|| { Regex::new("(?i)^(start|wine|wine64|wine-preloader).exe").unwrap()});
        if WINE_PARENT.is_match(p.as_str()) {
            env_lock.add(action!(
                EmulatorType::Wine,
                ScoreType::Process,
                s_add!("Suspicious parent process typical of Wine: ", WINE_PARENT.as_str()),
                7,
                0.85
            ));
        }

    }

    if !processes.is_empty() && processes.len() < 15 {
        let p_len = processes.len().to_string();
        env_lock.add(action!(
            EmulatorType::Wine,
            ScoreType::Process,
            s_add!("Low running process count (", p_len.as_str(), "), characteristic of Wine."),
            6,
            0.7
        ));
    }
    static WINE_PROCESS: LazyLock<Regex, fn() -> Regex> = LazyLock::new(|| {
        Regex::new("(?i)^(winedevice|wineboot|winedbg|winemine|wineconsole|wine-preloader).exe").unwrap()}
    );

    for p in processes.iter() {
        if WINE_PROCESS.is_match(p.as_str()) {
            env_lock.add(action!(
            EmulatorType::Wine,
            ScoreType::StrongFingerprint,
            s_add!("Discover Wine process: ", p.as_str()),
            8,
            0.8
        ));
        }
    }

    let wine_specific_files: [HeapStr; 18] = [
        // 可执行文件
        s!(r"\winecfg.exe").into(), // s!产生StackStr::<N>
        s!(r"\wineboot.exe").into(),
        s!(r"\winedbg.exe").into(),
        s!(r"\winemine.exe").into(),
        s!(r"\wineconsole.exe").into(),
        s!(r"\winedevice.exe").into(),
        // 图形/音频驱动
        s!(r"\winex11.drv").into(),
        s!(r"\winemac.drv").into(),
        s!(r"\winewayland.drv").into(),
        s!(r"\wineandroid.drv").into(),
        s!(r"\winepulse.drv").into(),
        s!(r"\winealsa.drv").into(),
        s!(r"\wineoss.drv").into(),
        s!(r"\winecoreaudio.drv").into(),
        s!(r"\winevulkan.dll").into(),
        // 系统驱动
        s!(r"\drivers\wineusb.sys").into(),
        s!(r"\drivers\winebus.sys").into(),
        s!(r"\drivers\winehid.sys").into(),
    ];

    for file in wine_specific_files {
        let path = s_add!(SYSTEM32.as_str(), file);
        let act = action!(EmulatorType::Wine, ScoreType::StrongFingerprint, 10, 0.95);
        if let Some(res_act) = __has_file(path, Some(32), act).await {
            env_lock.add(res_act);
        }
    }


    let unix_mapped_files: [HeapStr; 13] = [
        s!(r"Z:\etc\passwd").into(),
        s!(r"Z:\etc\fstab").into(),
        s!(r"Z:\etc\hosts").into(),
        s!(r"Z:\etc\shadow").into(),
        s!(r"Z:\etc\group").into(),
        s!(r"Z:\bin\bash").into(),
        s!(r"Z:\bin\sh").into(),
        s!(r"Z:\bin\ls").into(),
        s!(r"Z:\bin\cat").into(),
        s!(r"Z:\dev\null").into(),
        s!(r"Z:\dev\zero").into(),
        s!(r"Z:\proc\cpuinfo").into(),
        s!(r"Z:\proc\meminfo").into(),
    ];

    for file in unix_mapped_files {
        let act = action!(EmulatorType::Wine, ScoreType::File, 9, 0.7);
        if let Some(res_act) = __has_file(file, None, act).await {
            env_lock.add(res_act);
        }
    }

    let tlsh_targets: [(HeapStr, HeapStr); 4] = [
        (
            s!(r"\services.exe").into(),
            s!("T105C46B84BF8998DBCA1646B84CF7033513BDF68066979B074A5CF2210CB6FDC5E825E9").into()
        ),
        (
            s!(r"\plugplay.exe").into(),
            s!("T1A9F34AD977047CE7C698423E58EB5734133CF7C26A9287130964B72A0CA3AE06EE759D").into()
        ),
        (
            s!(r"\start.exe").into(),
            s!("T18E54D76077EE01D9F1F77A78997506240A3FFD90AA79C70E025D628D0F73A808DA5B63").into()
        ),
        (
            s!(r"\drivers\mountmgr.sys").into(),
            s!("T1BE946D90BB851C4BCA19437B4CB70F793339F68122478B0F1A18B26D1C96BDC6ED66D9").into()
        ),
    ];

    for (file, tlsh_hash) in tlsh_targets {
        let path = s_add!(SYSTEM32.as_str(), file.as_str());
        let act = action!(EmulatorType::Wine, ScoreType::FileContent, 9, 0.90);

        if let Some(res_act) = __file_diff(path, tlsh_hash, 20, act).await {
            env_lock.add(res_act);
        }
    }

    let wine_core_vars: [HeapStr; 7] = [
        s!("WINEPREFIX").into(),
        s!("WINELOADER").into(),
        s!("WINESERVER").into(),
        s!("WINEDEBUG").into(),
        s!("WINEARCH").into(),
        s!("WINEDLLPATH").into(),
        s!("WINEDLLOVERRIDES").into(),
    ];

    for kv in std::env::vars() {

        for var_name in &wine_core_vars {
            let act_wine = action!(EmulatorType::Wine, ScoreType::StrongFingerprint, 10, 0.99);
            if let Some(res_act) = __env_has_key(&kv, var_name.clone(), act_wine) {
                env_lock.add(res_act);
                break;
            }
        }


        let linux_keys: [HeapStr; 5] = [
            s!("DISPLAY").into(),
            s!("WAYLAND_DISPLAY").into(),
            s!("XDG_RUNTIME_DIR").into(),
            s!("XDG_DATA_HOME").into(),
            s!("LD_LIBRARY_PATH").into(),
        ];

        for key in linux_keys {
            let act_linux = action!(EmulatorType::Wine, ScoreType::Env, 7, 0.60);
            if let Some(res_act) = __env_has_key(&kv, key, act_linux) {
                env_lock.add(res_act);
            }
        }


        let unix_home_paths: [HeapStr; 2] = [
            s!("/home/").into(),
            s!("/Users/").into()
        ];
        let act_home = action!(EmulatorType::Wine, ScoreType::Env, 7, 0.6);
        if let Some(res_act) = __env_has_kv(&kv, s!("HOME"), &unix_home_paths, act_home) {
            env_lock.add(res_act);
        }

        let unix_bin_paths: [HeapStr; 3] = [
            s!("/usr/bin").into(),
            s!("/bin").into(),
            s!("/usr/local/bin").into()
        ];
        let act_path = action!(EmulatorType::Wine, ScoreType::Env, 7, 0.6);
        if let Some(res_act) = __env_has_kv(&kv, s!("PATH"), &unix_bin_paths, act_path) {
            env_lock.add(res_act);
        }
    }


    let kernel32_wine_funcs:[HeapStr; 5] = [
        s!("wine_get_unix_file_name").into(),
        s!("wine_get_dos_file_name").into(),
        s!("wine_get_version").into(),
        s!("wine_get_build_id").into(),
        s!("wine_get_host_version").into(),
    ];

    for func_name in kernel32_wine_funcs {
        if resolve::<usize>(ss!("kernel32.dll"), &*func_name).is_some() {
            env_lock.add(action!(
                EmulatorType::Wine,
                ScoreType::OtherSystemApi,
                s_add!("Found Wine specific export in kernel32.dll: ", func_name),
                10,
                1.0
            ));
        }
    }

    let ntdll_wine_funcs:[HeapStr; 4] = [
        s!("wine_server_call").into(),
        s!("wine_server_fd_to_handle").into(),
        s!("wine_nt_to_unix_file_name").into(),
        s!("wine_unix_to_nt_file_name").into(),
    ];

    for func_name in ntdll_wine_funcs {
        if resolve::<usize>(ss!("ntdll.dll"), &*func_name).is_some() {
            env_lock.add(action!(
                EmulatorType::Wine,
                ScoreType::OtherSystemApi,
                s_add!("Found Wine specific export in ntdll.dll: ", func_name),
                10,
                1.0
            ));
        }
    }

    type FnWineGetVersion = unsafe extern "C" fn() -> *const std::ffi::c_char;

    if let Some(func) = resolve::<FnWineGetVersion>(ss!("ntdll.dll"), ss!("wine_get_version")) {
        unsafe {
            let version_ptr = func(); // 调用该函数
            if !version_ptr.is_null() {
                // 将 C 字符串转换为 Rust 字符串
                if let Ok(version_str) = std::ffi::CStr::from_ptr(version_ptr).to_str() {
                    env_lock.add(action!(
                        EmulatorType::Wine,
                        ScoreType::StrongFingerprint,
                        s_add!("Wine version precisely detected: ", version_str),
                        10,
                        1.0
                    ));
                }
            }
        }
    }

    type FnWineGetBuildId = unsafe extern "C" fn() -> *const std::ffi::c_char;
    if let Some(func) = resolve::<FnWineGetBuildId>(ss!("ntdll.dll"), ss!("wine_get_build_id")) {
        unsafe {
            let build_id_ptr = func();
            if !build_id_ptr.is_null() {
                if let Ok(build_id_str) = std::ffi::CStr::from_ptr(build_id_ptr).to_str() {
                    env_lock.add(action!(
                        EmulatorType::Wine,
                        ScoreType::StrongFingerprint,
                        s_add!("Wine build ID detected: ", build_id_str),
                        10,
                        1.0
                    ));
                }
            }
        }
    }



}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::utils::fs::___tlsh_file_string;
    #[tokio::test]
    async fn ok() {
        sprint!(___tlsh_file_string(r#"C:\windows\system32\services.exe"#).await.unwrap());
        // T105C46B84BF8998DBCA1646B84CF7033513BDF68066979B074A5CF2210CB6FDC5E825E9
        sprint!(___tlsh_file_string(r#"C:\windows\system32\plugplay.exe"#).await.unwrap());
        // T1A9F34AD977047CE7C698423E58EB5734133CF7C26A9287130964B72A0CA3AE06EE759D
        sprint!(___tlsh_file_string(r#"C:\windows\system32\start.exe"#).await.unwrap());
        // T18E54D76077EE01D9F1F77A78997506240A3FFD90AA79C70E025D628D0F73A808DA5B63
        sprint!(___tlsh_file_string(r#"C:\windows\system32\drivers\mountmgr.sys"#).await.unwrap());
        // T1BE946D90BB851C4BCA19437B4CB70F793339F68122478B0F1A18B26D1C96BDC6ED66D9
    }

}