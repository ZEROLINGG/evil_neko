#![allow(unused)]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::shell::ExecResult;
use crate::utils::sys::info;




#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "server-db", derive(sqlx::FromRow))]
pub struct SystemInfo {
    pub hostname: Option<String>,
    pub username: Option<String>,

    pub pid: i64,
    pub process_path: String,

    pub user_permissions: Option<String>,

    pub os: String,

    pub os_version: Option<String>,
    pub os_build: Option<String>,

    pub arch: String,

    #[cfg_attr(feature = "server-db", sqlx(json))]
    pub cpu: Option<Vec<String>>,
    #[cfg_attr(feature = "server-db", sqlx(json))]
    pub gpu: Option<Vec<String>>,
    pub memory: Option<String>,
    #[cfg_attr(feature = "server-db", sqlx(json))]
    pub disk: Option<Vec<String>>,
    pub machine_id: Option<String>,
    pub systeminfo: Option<String>,
    #[cfg_attr(feature = "server-db", sqlx(json))]
    pub env: HashMap<String, String>,

    pub ip: String,

    pub network_info: Option<String>,
    pub running_processes: Option<String>,
}

impl Default for SystemInfo {
    fn default() -> SystemInfo {
        let env = info::collect_env();
        let pid = info::collect_pid();
        let process_path = info::collect_process_path();
        let os = info::collect_os();
        let arch = info::collect_arch();
        let ip = info::collect_ip();

        SystemInfo {
            hostname: None,
            username: None,
            pid,
            process_path,
            user_permissions: None,
            os,
            os_version: None,
            os_build: None,
            arch,
            cpu: None,
            gpu: None,
            memory: None,
            disk: None,
            machine_id: None,
            systeminfo: None,
            env,
            ip,
            network_info: None,
            running_processes: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Fingerprint {
    pub seed: u128,
    pub hash: u128,
    pub marker: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Nop { // 用于心跳和会话保活
    pub session_a: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Register {
    pub session_a: String,
    pub fingerprint: Fingerprint,
    pub system_info: SystemInfo,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Command{
    pub session_a: String,
    pub session_b: String,
    pub input: String,
    pub shell: String,
    pub timeout: Option<u64>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommandReturn{
    pub session_a: String,
    pub session_b: String,
    pub data: ExecResult,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShellSpawn{
    pub session_a: String,
    pub session_b: String,
    pub shell: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShellSend{
    pub session_a: String,
    pub session_b: String,
    pub input: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShellReturn{
    pub session_a: String,
    pub session_b: String,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
    pub is_close: bool,
}
