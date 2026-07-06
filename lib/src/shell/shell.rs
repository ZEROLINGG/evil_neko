//src/shell/shell.rs
#![cfg(feature = "shell")]
#![allow(unused)]
use anyhow::{anyhow, ensure, Result};
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot, Mutex, Notify, OnceCell};
use tokio::task::JoinHandle;
use crate::runtime::*;

#[cfg(unix)]
static BASH: OnceCell<Arc<Mutex<Shell>>> = OnceCell::const_new();
#[cfg(windows)]
static POWERSHELL: OnceCell<Arc<Mutex<Shell>>> = OnceCell::const_new();
#[cfg(unix)]
pub async fn bash() -> Arc<Mutex<Shell>> {
    BASH.get_or_init(|| async {
        Arc::new(Mutex::new(
            Shell::new("bash")
                .enable_output_buffer()
                .spawn()
                .await
                .unwrap()
        ))
    })
        .await
        .clone()
}
#[cfg(windows)]
pub async fn powershell() -> Arc<Mutex<Shell>> {
    POWERSHELL.get_or_init(|| async {
        Arc::new(Mutex::new(
            Shell::new(s!("powershell"))
                .enable_output_buffer()
                .spawn()
                .await
                .unwrap()
        ))
    })
        .await
        .clone()
}

#[cfg(windows)]
use std::os::windows::process::CommandExt;
use crate::shell::{decode_bytes, normalize_shell_name};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(windows)]
use crate::utils::win::resolve;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

type AsyncPreSendCallback =
Box<dyn FnMut(String) -> BoxFuture<'static, Option<String>> + Send + 'static>;
type AsyncOutputCallback =
Box<dyn FnMut(String) -> BoxFuture<'static, ()> + Send + 'static>;
type AsyncErrorCallback =
Box<dyn FnMut(String) -> BoxFuture<'static, ()> + Send + 'static>;
type AsyncExitCallback =
Box<dyn FnMut(Option<i32>) -> BoxFuture<'static, ()> + Send + 'static>;
type AsyncCloseCallback =
Box<dyn FnMut() -> BoxFuture<'static, ()> + Send + 'static>;

// ── 内部消息 ──────────────────────────────────────────────────────────────────

enum StdinMsg {
    Data(String),
    Close,
}

// ── Callbacks ─────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Callbacks {
    on_output: Option<AsyncOutputCallback>,
    on_error:  Option<AsyncErrorCallback>,
    on_exit:   Option<AsyncExitCallback>,
    on_close:  Option<AsyncCloseCallback>,
}

// ── Output Buffer ────────────────────────────────────────────────────────────

pub struct OutputBuffer {
    pub lines: Mutex<Vec<String>>,
    pub notify: Notify,
}

impl OutputBuffer {
    pub fn new() -> Self {
        Self {
            lines: Mutex::new(Vec::new()),
            notify: Notify::new(),
        }
    }
}

// ── ShellBuilder ──────────────────────────────────────────────────────────────

pub struct ShellBuilder {
    shell_path:           String,
    pre_send:             Option<AsyncPreSendCallback>,
    callbacks:            Callbacks,
    close_notify:         Arc<Notify>,
    enable_output_buffer: bool,
}

impl ShellBuilder {
    pub fn new(shell: impl Into<String>) -> Self {
        Self {
            shell_path:           shell.into(),
            pre_send:             None,
            callbacks:            Callbacks::default(),
            close_notify:         Arc::new(Notify::new()),
            enable_output_buffer: false,
        }
    }

    /// 启用内置缓存，以便后续能够使用 `shell.output(idle_time)`
    pub fn enable_output_buffer(mut self) -> Self {
        self.enable_output_buffer = true;
        self
    }

    pub fn on_send<F, Fut>(mut self, mut f: F) -> Self
    where
        F:   FnMut(String) -> Fut + Send + 'static,
        Fut: Future<Output = Option<String>> + Send + 'static,
    {
        self.pre_send = Some(Box::new(move |s| Box::pin(f(s))));
        self
    }

    pub fn on_output<F, Fut>(mut self, mut f: F) -> Self
    where
        F:   FnMut(String) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.callbacks.on_output = Some(Box::new(move |s| Box::pin(f(s))));
        self
    }

    pub fn on_error<F, Fut>(mut self, mut f: F) -> Self
    where
        F:   FnMut(String) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.callbacks.on_error = Some(Box::new(move |s| Box::pin(f(s))));
        self
    }

    pub fn on_exit<F, Fut>(mut self, mut f: F) -> Self
    where
        F:   FnMut(Option<i32>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.callbacks.on_exit = Some(Box::new(move |c| Box::pin(f(c))));
        self
    }

    pub fn on_close<F, Fut>(mut self, mut f: F) -> Self
    where
        F:   FnMut() -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.callbacks.on_close = Some(Box::new(move || Box::pin(f())));
        self
    }

    pub async fn spawn(self) -> Result<Shell> {
        let shell_path = self.shell_path.trim().to_string();
        ensure!(!shell_path.is_empty(), s!("shell path cannot be empty"));

        let pre_send  = Arc::new(Mutex::new(self.pre_send));
        let callbacks = Arc::new(Mutex::new(self.callbacks));

        let output_buffer = if self.enable_output_buffer {
            Some(Arc::new(OutputBuffer::new()))
        } else {
            None
        };

        let (tx_stdin, drop_tx, join) =
            Shell::spawn_process(&shell_path, callbacks.clone(), output_buffer.clone()).await?;

        Ok(Shell {
            shell_path,
            tx_stdin,
            drop_tx: Some(drop_tx),
            pre_send,
            callbacks,
            join: Some(join),
            droped: false,
            close_notify: self.close_notify,
            output_buffer,
        })
    }
}

// ── Shell ─────────────────────────────────────────────────────────────────────

pub struct Shell {
    shell_path:    String,
    tx_stdin:      mpsc::Sender<StdinMsg>,
    drop_tx:       Option<oneshot::Sender<()>>,
    pre_send:      Arc<Mutex<Option<AsyncPreSendCallback>>>,
    callbacks:     Arc<Mutex<Callbacks>>,
    join:          Option<JoinHandle<()>>,
    droped:        bool,
    close_notify:  Arc<Notify>,
    output_buffer: Option<Arc<OutputBuffer>>,
}

impl Shell {
    // ── 公开 API ──────────────────────────────────────────────────────────────

    pub fn new(shell: impl Into<String>) -> ShellBuilder {
        ShellBuilder::new(shell)
    }


    pub async fn output(&mut self, idle_time: Option<Duration>) -> String {
        if self.output_buffer.is_none() { return String::new() }


        let timeout_duration = idle_time.unwrap_or(Duration::from_millis(200));
        let ob = self.output_buffer.as_ref().unwrap();

        loop {
            tokio::select! {
                _ = self.close_notify.notified() => {
                    break;
                }

                res = tokio::time::timeout(timeout_duration, ob.notify.notified()) => {
                    match res {
                        Ok(_) => {
                            continue;
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }
            }
        }

        let mut lock = ob.lines.lock().await;
        std::mem::take(&mut *lock).join("\n")
    }

    /// 发送原始内容到 stdin（不附加换行）。
    pub async fn send(&mut self, cmd: &str) -> Result<()> {
        ensure!(!self.droped, s!("shell is closed"));

        // 拦截控制字符 (如 "^C", "^D", "^R")
        if cmd.len() < 5 {
            let trimmed = cmd.trim();
            if trimmed.chars().count() == 2 {
                let mut chars = trimmed.chars();
                if let (Some('^'), Some(c)) = (chars.next(), chars.next()) {
                    let char = c.to_ascii_uppercase();
                    if "CDR".contains(char) {
                        return self.send_control_char(char).await;
                    }
                }
            }
        }

        if let Some(s) = self.preprocess_send(cmd.to_string()).await {
            self.tx_stdin
                .send(StdinMsg::Data(s))
                .await
                .map_err(|_| anyhow!(s!("send failed: stdin channel closed")))?;
        }
        Ok(())
    }

    /// 发送一行命令（自动附加 `\n`）。
    pub async fn send_line(&mut self, cmd: &str) -> Result<()> {
        self.send(&format!("{cmd}\n")).await
    }

    pub async fn send_control_char(&mut self, ctrl: char) -> Result<()> {
        match ctrl {
            'R' => {
                self.reset().await
            }
            'C' => {
                let ctrl_c = "\x03".to_string();
                self.tx_stdin
                    .send(StdinMsg::Data(ctrl_c))
                    .await
                    .map_err(|_| anyhow!(s!("send ^C failed: stdin channel closed")))
            }
            'D' => {
                let ctrl_d = "\x04".to_string();
                self.tx_stdin
                    .send(StdinMsg::Data(ctrl_d))
                    .await
                    .map_err(|_| anyhow!(s!("send ^D failed: stdin channel closed")))
            }
            _ => {
                Ok(())
            }
        }
    }


    /// 等待到 close/drop 调用，不受单次 exit 和进程崩溃影响
    pub async fn join(&mut self) -> Result<()> {
        if !self.droped {
            self.close_notify.notified().await;
        }

        if let Some(handle) = self.join.take() {
            let _ = handle.await;
        }
        Ok(())
    }

    /// 等待单次进程 shell 解释器退出就返回
    pub async fn join_exit(&mut self) -> Result<()> {
        if let Some(handle) = self.join.take() {
            handle.await.map_err(|e| anyhow!("{}{e}",s!("join_exit failed: ")))?;
        }
        Ok(())
    }

    /// 关闭当前进程并用相同参数重启；所有回调保持不变。
    pub async fn reset(&mut self) -> Result<()> {
        // 已关闭则不允许 reset（close 是不可逆操作）
        ensure!(!self.droped, s!("shell is closed"));
        self.exit().await?;
        if let Some(handle) = self.join.take() {
            let _ = handle.await;
        }

        let (tx_stdin, drop_tx, join) =
            Self::spawn_process(&self.shell_path, self.callbacks.clone(), self.output_buffer.clone()).await?;
        self.tx_stdin = tx_stdin;
        self.drop_tx  = Some(drop_tx);
        self.join     = Some(join);

        self.droped = false;
        Ok(())
    }

    async fn preprocess_send(&self, raw: String) -> Option<String> {
        let mut guard = self.pre_send.lock().await;
        if let Some(f) = guard.as_mut() {
            f(raw).await
        } else {
            Some(raw)
        }
    }

    /// 退出已打开的 shell 解释器
    pub async fn exit(&mut self) -> Result<()> {
        let exit_cmd = Self::exit_command(&self.shell_path);
        let _ = self.tx_stdin.send(StdinMsg::Data(exit_cmd)).await;
        let _ = self.tx_stdin.send(StdinMsg::Close).await;
        self.join_exit().await
    }

    /// 手动强制关闭 Shell 实例，不允许继续重置
    pub fn close(&mut self) -> Result<()> {
        if self.droped {
            return Ok(());
        }
        self.droped = true;

        let _ = self.drop_tx.take();
        let _ = self.tx_stdin.try_send(StdinMsg::Close);

        self.close_notify.notify_waiters();

        Ok(())
    }

    async fn spawn_process(
        shell: &str,
        callbacks: Arc<Mutex<Callbacks>>,
        output_buffer: Option<Arc<OutputBuffer>>,
    ) -> Result<(mpsc::Sender<StdinMsg>, oneshot::Sender<()>, JoinHandle<()>)> {
        let shell_name = normalize_shell_name(shell)?;
        let mut cmd    = build_command(shell, &shell_name)?;

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let mut child = cmd.spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        let stdout    = child.stdout.take().unwrap();
        let stderr    = child.stderr.take().unwrap();

        let (tx_stdin, mut rx_stdin) = mpsc::channel::<StdinMsg>(32);
        let (drop_tx, drop_rx)   = oneshot::channel::<()>();

        let init_cmd = init_command(&shell_name);
        if let Some(cmd) = init_cmd {
            stdin.write_all(cmd.as_bytes()).await?;
            stdin.flush().await?;
        }

        // ── stdout task ───────────────────────────────────────────────────────
        let cb_stdout = callbacks.clone();
        let ob_stdout = output_buffer.clone();
        let stdout_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut buf = Vec::new();

            while let Ok(n) = reader.read_until(b'\n', &mut buf).await {
                if n == 0 {
                    break; // EOF
                }

                let decoded = decode_bytes(&buf);
                let line = decoded.trim_end_matches(&['\r', '\n'][..]).to_string();

                buf.clear();

                // 写入缓存区并通知
                if let Some(ob) = &ob_stdout {
                    let mut lock = ob.lines.lock().await;
                    lock.push(line.clone());
                    ob.notify.notify_one();
                }

                let fut_opt = {
                    let mut cb = cb_stdout.lock().await;
                    cb.on_output.as_mut().map(|f| f(line))
                };
                if let Some(fut) = fut_opt {
                    fut.await;
                }
            }
        });

        // ── stderr task ───────────────────────────────────────────────────────
        let cb_stderr = callbacks.clone();
        let ob_stderr = output_buffer.clone();
        let stderr_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut buf = Vec::new();

            while let Ok(n) = reader.read_until(b'\n', &mut buf).await {
                if n == 0 {
                    break;
                }

                let decoded = decode_bytes(&buf);
                let line = decoded.trim_end_matches(&['\r', '\n'][..]).to_string();

                buf.clear();

                // stderr也写入缓存区并通知
                if let Some(ob) = &ob_stderr {
                    let mut lock = ob.lines.lock().await;
                    lock.push(line.clone());
                    ob.notify.notify_one();
                }

                let fut_opt = {
                    let mut cb = cb_stderr.lock().await;
                    cb.on_error.as_mut().map(|f| f(line))
                };
                if let Some(fut) = fut_opt {
                    fut.await;
                }
            }
        });

        // ── stdin task ────────────────────────────────────────────────────────
        let stdin_task = tokio::spawn(async move {
            while let Some(msg) = rx_stdin.recv().await {
                match msg {
                    StdinMsg::Close => break,
                    StdinMsg::Data(data) => {
                        if stdin.write_all(data.as_bytes()).await.is_err() {
                            break;
                        }
                        if stdin.flush().await.is_err() {
                            break;
                        }
                    }
                }
            }
            drop(stdin);
        });

        // ── 主监控 task ───────────────────────────────────────────────────────
        let cb_main = callbacks.clone();
        let join = tokio::spawn(async move {
            let _close_code: Option<i32> = tokio::select! {
                status = child.wait() => {
                    let code = status.ok().and_then(|s| s.code());
                    let fut_opt = {
                        let mut cb = cb_main.lock().await;
                        cb.on_exit.as_mut().map(|f| f(code))
                    };
                    if let Some(fut) = fut_opt {
                        fut.await;
                    }
                    code
                }
                _ = drop_rx => {
                    if let Err(e) = child.kill().await {
                        eprintln!("{}{e}",s!("kill failed (process may have already exited): "));
                    }
                    child.wait().await.ok().and_then(|s| s.code())
                }
            };

            let _ = tokio::join!(stdout_task, stderr_task, stdin_task);

            let fut_opt = {
                let mut cb = cb_main.lock().await;
                cb.on_close.as_mut().map(|f| f())
            };
            if let Some(fut) = fut_opt {
                fut.await;
            }
        });

        Ok((tx_stdin, drop_tx, join))
    }

    fn exit_command(shell_path: &str) -> String {
        let shell_name = normalize_shell_name(shell_path).unwrap_or_default();
        let name_str = shell_name.as_str();
        if name_str == ss!("python") {
            sss!("quit()\n")
        } else if name_str == ss!("node") {
            sss!(".exit\n")
        } else if name_str == ss!("powershell") || name_str == ss!("pwsh") {
            sss!("exit\n")
        } else {
            sss!("exit\n")
        }
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

// ── build_command / utf8_init_command ────────────────────────────────

fn build_command(shell: &str, shell_name: &str) -> Result<Command> {
    let mut cmd = Command::new(shell);

    if shell_name == ss!("bash") {
        cmd.args([sss!("--norc"), sss!("--noprofile"), sss!("-s")]);
    } else if shell_name == ss!("zsh") {
        cmd.args([sss!("-f"), sss!("-i")]);
    } else if shell_name == ss!("sh") {
        cmd.arg(sss!("-i"));
    } else if shell_name == ss!("fish") {
        cmd.args([sss!("--no-config"), sss!("-i")]);
    } else if shell_name == ss!("cmd") {
        cmd.args([sss!("/Q"), sss!("/K"), sss!("prompt $G")]);
    } else if shell_name == ss!("powershell") || shell_name == ss!("pwsh") {
        cmd.args([
            sss!("-ep"),
            sss!("Bypass"),
            sss!("-NoExit"),
            sss!("-NoProfile"),
            sss!("-NonInteractive"),
            sss!("-Command"),
            "-".to_string()
        ]);
    } else if shell_name == ss!("python") {
        cmd.args([sss!("-u"), sss!("-i")]);
    } else if shell_name == ss!("node") {
        cmd.arg(sss!("-i"));
    } else {
        anyhow::bail!("{}{shell_name}",s!("unsupported shell: "));
    }

    Ok(cmd)
}


fn init_command(shell_name: &str) -> Option<String> {
    if shell_name == ss!("cmd") {
        Some(sss!("chcp 65001 >nul 2>&1\r\n"))
    } else if shell_name == ss!("powershell") || shell_name == ss!("pwsh") {
        Some(sss!(
            "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
             [Console]::InputEncoding  = [System.Text.Encoding]::UTF8; \
             $OutputEncoding           = [System.Text.Encoding]::UTF8\n"
        ))
    } else {
        None
    }
}


// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn test_shell_basic() {
        let mut shell = Shell::new("python")
            .on_output(async move |s| println!("{s}"))
            .on_error(async move |s| eprintln!("stderr: {s}"))
            .on_exit(async move |code| eprintln!("exit: {:?}", code))
            .on_close(async move || println!("[close]"))
            .spawn()
            .await
            .unwrap();

        shell.send_line("print(111)").await.unwrap();
        shell.send_line("lllll").await.unwrap();

        shell.exit().await.unwrap();
        shell.close().unwrap();

        let _ = shell.join().await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_reset2() {
        let mut shell = Shell::new("bash")
            .on_output(async move |s| println!("> {s}"))
            .on_exit(async move |code| eprintln!("exit: {:?}", code))
            .spawn()
            .await
            .unwrap();

        shell.send_line("echo 111;cd ..;pwd").await.unwrap();
        shell.send_line("cat").await.unwrap(); // 错误命令导致bash阻塞
        shell.send_line("^R").await.unwrap();
        shell.send_line("echo 222").await.unwrap();
        shell.send_line("whoami").await.unwrap();
        shell.exit().await.unwrap();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_reset3() {
        let mut shell = Shell::new("powershell")
            .on_output(async move |s| println!("> {s}"))
            .on_exit(async move |code| eprintln!("exit: {:?}", code))
            .spawn()
            .await
            .unwrap();

        shell.send_line("echo 111;cd ..;pwd").await.unwrap();
        shell.send_line("systeminfo").await.unwrap();
        shell.send_line("^R").await.unwrap();
        shell.send_line("echo 哈哈哈哈").await.unwrap();
        shell.send_line("whoami").await.unwrap();
        shell.exit().await.unwrap();
    }
}