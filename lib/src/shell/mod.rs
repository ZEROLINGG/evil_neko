//lib/src/shell/mod.rs
#![allow(unused)]
mod ppty;
mod pty;

use anyhow::{Result, anyhow};
use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::process::Command;
use crate::{s,ss,s_fmt};


static SHELL_NAME_REGEX: OnceLock<regex::Regex> = OnceLock::new();
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[derive(Debug)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

fn normalize_shell_name(shell: &str) -> Result<String> {
    let re = SHELL_NAME_REGEX.get_or_init(|| regex::Regex::new(r"[\d.].*$").unwrap());

    let name = Path::new(shell)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .ok_or_else(|| anyhow!("{}{shell}",ss!("invalid shell path: ")))?;

    Ok(re.replace(&name, "").into_owned())
}

pub async fn exec(input: &str, shell: &str, timeout_dur: Option<Duration>) -> Result<ExecResult> {
    let shell = shell.trim();
    anyhow::ensure!(!shell.is_empty(), s!("shell path cannot be empty"));
    
    let shell_name = normalize_shell_name(shell)?;

    let mut cmd = build_command(shell, &shell_name, input)?;
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    match timeout_dur {
        None => {
            let child = cmd.spawn()?;
            let output = child.wait_with_output().await?;
            Ok(to_exec_result(output))
        }
        Some(dur) => {
            let mut child = cmd.spawn()?;
            let stdout_handle = child.stdout.take();
            let stderr_handle = child.stderr.take();

            let read_fut = async {
                use tokio::io::AsyncReadExt;
                let mut stdout = String::new();
                let mut stderr = String::new();
                tokio::join!(
                    async {
                        if let Some(mut h) = stdout_handle {
                            let _ = h.read_to_string(&mut stdout).await;
                        }
                    },
                    async {
                        if let Some(mut h) = stderr_handle {
                            let _ = h.read_to_string(&mut stderr).await;
                        }
                    },
                );
                (stdout, stderr)
            };

            tokio::select! {
                (status, (stdout, stderr)) = async { (child.wait().await, read_fut.await) } => {
                    Ok(ExecResult {
                        stdout,
                        stderr,
                        exit_code: status?.code().unwrap_or(-1),
                    })
                }
                _ = tokio::time::sleep(dur) => {
                    let _ = child.kill().await;
                    Err(anyhow!("{}{dur:?}",ss!("command timed out after ")))
                }
            }
        }
    }
}

fn build_command(shell: &str, shell_name: &str, input: &str) -> Result<Command> {
    let mut cmd = Command::new(shell);

    if shell_name == ss!("sh") || shell_name == ss!("zsh") || shell_name == ss!("bash") || shell_name == ss!("fish") {
        cmd.args([s!("-c"), input.to_string()]);
    } else if shell_name == ss!("node") {
        cmd.args([s!("-e"), input.to_string()]);
    } else if shell_name == ss!("python") {
        #[cfg(target_os = "windows")]
        cmd.env(s!("PYTHONUTF8"), "1");
        cmd.args([s!("-c"), input.to_string()]);
    } else if shell_name == ss!("cmd") {
        #[cfg(not(target_os = "windows"))]
        cmd.args([s!("/C"), input.to_string()]);
        #[cfg(target_os = "windows")]
        {
            let wrapped = format!("{}{}", ss!("chcp 65001 >nul 2>&1 & "), input);
            cmd.args([s!("/C"), wrapped]);
        }
    } else if shell_name == ss!("powershell") || shell_name == ss!("pwsh") {
        #[cfg(not(target_os = "windows"))]
        cmd.args([s!("-ep"), s!("Bypass"), s!("-nop"), s!("-c"), input.to_string()]);
        #[cfg(target_os = "windows")]
        {
            let wrapped = format!(
                "{}{}",
                ss!(r#"[Console]::OutputEncoding = [System.Text.Encoding]::UTF8;
                 [Console]::InputEncoding  = [System.Text.Encoding]::UTF8;
                 $OutputEncoding           = [System.Text.Encoding]::UTF8;
                 "#),
                input
            );
            cmd.args([s!("-ep"), s!("Bypass"), s!("-nop"), s!("-c"), wrapped]);
        }
    } else {
        anyhow::bail!("{}{shell_name}", s!("unsupported shell: "));
    }

    Ok(cmd)
}

#[inline]
fn to_exec_result(output: std::process::Output) -> ExecResult {
    ExecResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
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

    #[cfg(unix)]
    #[tokio::test]
    async fn test_timeout() {
        let result = exec("sleep 10", "bash", Some(Duration::from_millis(200))).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
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
    }
}
