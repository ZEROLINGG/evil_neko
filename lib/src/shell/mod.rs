//lib/src/shell/mod.rs
#![allow(unused)]
pub mod shell;
mod pty;

use std::borrow::Cow;
use anyhow::{Result, anyhow, bail};
use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use crate::runtime::*;

static SHELL_NAME_REGEX: OnceLock<regex::Regex> = OnceLock::new();

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(windows)]
use crate::utils::win::resolve;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl ExecResult {
    #[inline]
    pub fn ok(self) -> Result<String> {
        if self.exit_code == 0 { return Ok(self.stdout) }
        bail!(self.stderr)
    }

    #[inline]
    pub fn success(&self) -> bool { self.exit_code == 0 }

    #[inline]
    pub fn failed(&self) -> bool { !self.success() }
}

fn normalize_shell_name(shell: &str) -> Result<String> {
    let re = SHELL_NAME_REGEX.get_or_init(|| regex::Regex::new(r"[\d.].*$").unwrap());

    let name = Path::new(shell)
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("{} {}", ss!("invalid shell path:"), shell))?
        .to_lowercase();

    Ok(re.replace(&name, "").into_owned())
}

pub async fn exec<'a>(
    input: impl Into<Cow<'a, str>>,
    shell: impl Into<Cow<'a, str>>,
    timeout_dur: Option<Duration>
) -> Result<ExecResult> {
    let shell = shell.into();
    let shell = shell.trim();
    anyhow::ensure!(!shell.is_empty(), s!("shell path cannot be empty"));

    let shell_name = normalize_shell_name(shell)?;
    let input_ref = input.into();

    let mut cmd = build_command(shell, &shell_name, input_ref.as_ref())?;

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = if let Some(dur) = timeout_dur {
        tokio::time::timeout(dur, cmd.spawn()?.wait_with_output())
            .await
            .map_err(|_| anyhow!("{ } {:?} ", ss!("command timed out after"), dur))??
    } else {
        cmd.spawn()?.wait_with_output().await?
    };

    Ok(to_exec_result(output))
}

fn build_command(shell: &str, shell_name: &str, input: &str) -> Result<Command> {
    let mut cmd = Command::new(shell);

    match shell_name {
        "sh" | "zsh" | "bash" | "fish" => {
            cmd.args([sss!("-c"), input.to_string()]);
        }
        "node" => {
            cmd.args([sss!("-e"), input.to_string()]);
        }
        "python" => {
            #[cfg(target_os = "windows")]
            cmd.env(sss!("PYTHONUTF8"), sss!("1"));

            cmd.args([sss!("-c"), input.to_string()]);
        }
        "cmd" => {
            #[cfg(not(target_os = "windows"))]
            cmd.args([sss!("/C"), input.to_string()]);

            #[cfg(target_os = "windows")]
            {
                let wrapped = format!("{}{}", ss!("chcp 65001 >nul 2>&1 & "), input.to_string());
                cmd.args([sss!("/C"), wrapped]);
            }
        }
        "powershell" | "pwsh" => {
            #[cfg(not(target_os = "windows"))]
            cmd.args([sss!("-ep"), sss!("Bypass"), sss!("-nop"), sss!("-c"), input.to_string()]);

            #[cfg(target_os = "windows")]
            {
                let wrapped = format!(
                    "{}{}",
                    ss!(r#"[Console]::OutputEncoding = [System.Text.Encoding]::UTF8;
                 [Console]::InputEncoding  = [System.Text.Encoding]::UTF8;
                 $OutputEncoding           = [System.Text.Encoding]::UTF8;
                 "#),
                    input.to_string()
                );
                cmd.args([sss!("-ep"), sss!("Bypass"), sss!("-nop"), sss!("-c"), wrapped]);
            }
        }
        _ => {
            bail!("{}{}", s!("unsupported shell: "), shell_name);
        }
    }

    Ok(cmd)
}

#[inline]
pub fn decode_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }

    #[cfg(windows)]
    {
        return decode_windows_native(bytes);
    }

    #[cfg(not(windows))]
    {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

#[cfg(windows)]
fn decode_windows_native(bytes: &[u8]) -> String {
    type FnGetConsoleOutputCP = unsafe extern "system" fn() -> u32;
    type FnMultiByteToWideChar = unsafe extern "system" fn(
        code_page: u32,
        dw_flags: u32,
        lp_multi_byte_str: *const u8,
        cb_multi_byte: i32,
        lp_wide_char_str: *mut u16,
        cch_wide_char: i32,
    ) -> i32;

    unsafe {
        let get_console_output_cp: Option<FnGetConsoleOutputCP> =
            resolve(ss!("kernel32.dll"), ss!("GetConsoleOutputCP"));
        let multi_byte_to_wide_char: Option<FnMultiByteToWideChar> =
            resolve(ss!("kernel32.dll"), ss!("MultiByteToWideChar"));

        if let (Some(get_cp), Some(mb_to_wc)) = (get_console_output_cp, multi_byte_to_wide_char) {
            let mut cp = get_cp();
            if cp == 0 {
                cp = 0; // CP_ACP 兜底
            }

            let required_size = mb_to_wc(
                cp,
                0,
                bytes.as_ptr(),
                bytes.len() as i32,
                std::ptr::null_mut(),
                0,
            );

            if required_size > 0 {
                let mut wide_chars: Vec<u16> = vec![0; required_size as usize];

                let converted_size = mb_to_wc(
                    cp,
                    0,
                    bytes.as_ptr(),
                    bytes.len() as i32,
                    wide_chars.as_mut_ptr(),
                    required_size,
                );

                if converted_size > 0 {
                    return String::from_utf16_lossy(&wide_chars);
                }
            }
        }
    }

    String::from_utf8_lossy(bytes).into_owned()
}

#[inline]
fn to_exec_result(output: std::process::Output) -> ExecResult {
    ExecResult {
        stdout: decode_bytes(&output.stdout),
        stderr: decode_bytes(&output.stderr),
        exit_code: output.status.code().unwrap_or(-1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    async fn test(shell: &str) {
        let out = exec("ls", shell, None).await.unwrap().stdout;
        assert!(out.contains("Cargo.toml"));
        println!("{}", out);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_shell() {
        let shells = vec!["zsh", "bash", "sh", "pwsh"];
        for shell in shells {
            test(shell).await;
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_windows_utf8() {
        let result = exec("echo 你好世界", "cmd", None).await.unwrap();
        println!("---\ncmd: {}\n---", result.stdout.trim());
        assert!(
            result.stdout.contains("你好世界"),
            "cmd 中文乱码: {:?}",
            result.stdout
        );

        // 测试容易产生乱码的系统组件命令
        let result2 = exec("ipconfig", "cmd", None).await.unwrap();
        println!("---\nipconfig: {}\n---", result2.stdout.trim());
        assert!(
            !result2.stdout.contains('\u{FFFD}'),
            "ipconfig 中存在乱码字符: {:?}",
            result2.stdout
        );
    }
}