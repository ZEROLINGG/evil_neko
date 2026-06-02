#![cfg(feature = "pty")]
#![allow(unused)]
//
// use anyhow::{anyhow, ensure, Result};
// use std::future::Future;
// use std::pin::Pin;
// use std::sync::Arc;
// use std::io::{Read, Write};
// use tokio::sync::{mpsc, oneshot, Mutex, Notify};
// use tokio::task::JoinHandle;
// use portable_pty::{CommandBuilder, native_pty_system, PtySize, Child, MasterPty};
// use tokio::process::Command;
// use crate::{s, ss, s_fmt};
// use crate::shell::normalize_shell_name;
//
// type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
//
// type AsyncPreSendCallback = Box<dyn FnMut(String) -> BoxFuture<'static, Option<String>> + Send + 'static>;
// type AsyncOutputCallback  = Box<dyn FnMut(String) -> BoxFuture<'static, ()> + Send + 'static>;
// type AsyncExitCallback    = Box<dyn FnMut(Option<i32>) -> BoxFuture<'static, ()> + Send + 'static>;
// type AsyncCloseCallback   = Box<dyn FnMut() -> BoxFuture<'static, ()> + Send + 'static>;
//
// // ── 内部消息 ──────────────────────────────────────────────────────────────────
//
// enum StdinMsg {
//     Data(String),
//     Resize(u16, u16), // cols, rows
//     Close,
// }
//
// #[derive(Default)]
// struct Callbacks {
//     on_output: Option<AsyncOutputCallback>,
//     on_exit:   Option<AsyncExitCallback>,
//     on_close:  Option<AsyncCloseCallback>,
// }
//
// // ── PtyShellBuilder ───────────────────────────────────────────────────────────
//
// pub struct PtyShellBuilder {
//     shell_path:   String,
//     cols:         u16,
//     rows:         u16,
//     pre_send:     Option<AsyncPreSendCallback>,
//     callbacks:    Callbacks,
//     close_notify: Arc<Notify>,
// }
//
// impl PtyShellBuilder {
//     pub fn new(shell: impl Into<String>) -> Self {
//         Self {
//             shell_path:   shell.into(),
//             cols:         80,  // 默认终端大小
//             rows:         24,
//             pre_send:     None,
//             callbacks:    Callbacks::default(),
//             close_notify: Arc::new(Notify::new()),
//         }
//     }
//
//     pub fn with_size(mut self, cols: u16, rows: u16) -> Self {
//         self.cols = cols;
//         self.rows = rows;
//         self
//     }
//
//     pub fn on_send<F, Fut>(mut self, mut f: F) -> Self
//     where
//         F:   FnMut(String) -> Fut + Send + 'static,
//         Fut: Future<Output = Option<String>> + Send + 'static,
//     {
//         self.pre_send = Some(Box::new(move |s| Box::pin(f(s))));
//         self
//     }
//
//     pub fn on_output<F, Fut>(mut self, mut f: F) -> Self
//     where
//         F:   FnMut(String) -> Fut + Send + 'static,
//         Fut: Future<Output = ()> + Send + 'static,
//     {
//         self.callbacks.on_output = Some(Box::new(move |s| Box::pin(f(s))));
//         self
//     }
//
//     pub fn on_exit<F, Fut>(mut self, mut f: F) -> Self
//     where
//         F:   FnMut(Option<i32>) -> Fut + Send + 'static,
//         Fut: Future<Output = ()> + Send + 'static,
//     {
//         self.callbacks.on_exit = Some(Box::new(move |c| Box::pin(f(c))));
//         self
//     }
//
//     pub fn on_close<F, Fut>(mut self, mut f: F) -> Self
//     where
//         F:   FnMut() -> Fut + Send + 'static,
//         Fut: Future<Output = ()> + Send + 'static,
//     {
//         self.callbacks.on_close = Some(Box::new(move || Box::pin(f())));
//         self
//     }
//
//     pub async fn spawn(self) -> Result<PtyShell> {
//         let shell_path = self.shell_path.trim().to_string();
//         ensure!(!shell_path.is_empty(), s!("shell path cannot be empty"));
//
//         let pre_send  = Arc::new(Mutex::new(self.pre_send));
//         let callbacks = Arc::new(Mutex::new(self.callbacks));
//
//         let (tx_stdin, drop_tx, join) = PtyShell::spawn_pty(
//             &shell_path,
//             self.cols,
//             self.rows,
//             callbacks.clone()
//         ).await?;
//
//         Ok(PtyShell {
//             shell_path,
//             tx_stdin,
//             drop_tx: Some(drop_tx),
//             pre_send,
//             callbacks,
//             join: Some(join),
//             droped: false,
//             close_notify: self.close_notify,
//         })
//     }
// }
//
// // ── PtyShell ──────────────────────────────────────────────────────────────────
// pub struct PtyShell {
//     shell_path:   String,
//     tx_stdin:     mpsc::Sender<StdinMsg>,
//     drop_tx:      Option<oneshot::Sender<()>>,
//     pre_send:     Arc<Mutex<Option<AsyncPreSendCallback>>>,
//     callbacks:    Arc<Mutex<Callbacks>>,
//     join:         Option<JoinHandle<()>>,
//     droped:       bool,
//     close_notify: Arc<Notify>,
// }
//
// impl PtyShell {
//     pub fn new(shell: impl Into<String>) -> PtyShellBuilder {
//         PtyShellBuilder::new(shell)
//     }
//
//     pub async fn send(&mut self, cmd: &str) -> Result<()> {
//         ensure!(!self.droped, s!("pty is closed"));
//         if let Some(s) = self.preprocess_send(cmd.to_string()).await {
//             self.send_raw(&s).await?;
//         }
//         Ok(())
//     }
//
//     pub async fn send_line(&mut self, cmd: &str) -> Result<()> {
//         // PTY 规范通常推荐发送 \r，驱动程序通常会将 \r 转换为 shell 能理解的换行
//         self.send(&format!("{}\r", cmd)).await
//     }
//
//     async fn send_raw(&mut self, data: &str) -> Result<()> {
//         self.tx_stdin
//             .send(StdinMsg::Data(data.to_string()))
//             .await
//             .map_err(|_| anyhow!(s!("send failed: stdin channel closed")))
//     }
//
//     pub async fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
//         self.tx_stdin
//             .send(StdinMsg::Resize(cols, rows))
//             .await
//             .map_err(|_| anyhow!(s!("resize failed")))
//     }
//
//     async fn preprocess_send(&self, raw: String) -> Option<String> {
//         let mut guard = self.pre_send.lock().await;
//         if let Some(f) = guard.as_mut() {
//             f(raw).await
//         } else {
//             Some(raw)
//         }
//     }
//     async fn exit(&mut self) -> Result<()> {
//         let cmd = exit_command(self.shell_path.as_str());
//         self.send_raw(cmd.as_str()).await
//     }
//
//     pub fn close(&mut self) -> Result<()> {
//         if self.droped { return Ok(()); }
//         self.droped = true;
//         let _ = self.drop_tx.take();
//         let _ = self.tx_stdin.try_send(StdinMsg::Close);
//         self.close_notify.notify_waiters();
//         Ok(())
//     }
//
//     pub async fn join(&mut self) -> Result<()> {
//         if let Some(handle) = self.join.take() {
//             let _ = handle.await;
//         }
//         Ok(())
//     }
//
//     async fn spawn_pty(
//         shell: &str,
//         cols: u16,
//         rows: u16,
//         callbacks: Arc<Mutex<Callbacks>>,
//     ) -> Result<(mpsc::Sender<StdinMsg>, oneshot::Sender<()>, JoinHandle<()>)> {
//         let shell_name = normalize_shell_name(shell).unwrap_or_else(|_| shell.to_string());
//         let pty_system = native_pty_system();
//
//         let pair = pty_system.openpty(PtySize {
//             rows,
//             cols,
//             pixel_width: 0,
//             pixel_height: 0,
//         })?;
//
//         let mut cmd_builder = build_command_builder(&shell, &shell_name)?;
//
//         let child = pair.slave.spawn_command(cmd_builder)?;
//         drop(pair.slave);
//
//         let mut reader = pair.master.try_clone_reader()?;
//         let mut writer = pair.master.take_writer()?;
//         let master = pair.master;
//
//         let (tx_stdin, mut rx_stdin) = mpsc::channel::<StdinMsg>(32);
//         let (drop_tx, mut drop_rx)   = oneshot::channel::<()>();
//
//         let init_cmd = init_command(&shell_name);
//         if let Some(cmd) = init_cmd {
//             tx_stdin
//                 .send(StdinMsg::Data(cmd))
//                 .await
//                 .map_err(|_| anyhow!(s!("send failed: init_cmd send failed")))?
//         }
//
//         // ── 3. 读取任务 (同步桥接异步) ───────────────────────────────────────
//         let cb_out = callbacks.clone();
//         let (tx_out, mut rx_out) = mpsc::channel::<String>(100);
//
//         // 创建纯 OS 线程阻塞读取 (避免阻塞 Tokio executor)
//         std::thread::spawn(move || {
//             let mut buf = [0u8; 2048];
//             loop {
//                 match reader.read(&mut buf) {
//                     Ok(n) if n > 0 => {
//                         let text = String::from_utf8_lossy(&buf[..n]).into_owned();
//                         if tx_out.blocking_send(text).is_err() {
//                             break; // 接收端关闭
//                         }
//                     }
//                     _ => break, // EOF 或错误
//                 }
//             }
//         });
//
//         // 将同步读到的字符串派发给异步回调
//         let out_task = tokio::spawn(async move {
//             while let Some(text) = rx_out.recv().await {
//                 let fut = {
//                     let mut cb = cb_out.lock().await;
//                     cb.on_output.as_mut().map(|f| f(text))
//                 };
//                 if let Some(fut) = fut { fut.await; }
//             }
//         });
//
//         // ── 4. 写入 & 控制任务 ───────────────────────────────────────────────
//         let stdin_task = tokio::spawn(async move {
//             while let Some(msg) = rx_stdin.recv().await {
//                 match msg {
//                     StdinMsg::Close => break,
//                     StdinMsg::Data(data) => {
//                         if writer.write_all(data.as_bytes()).is_err() {
//                             break;
//                         }
//                         let _ = writer.flush();
//                     }
//                     StdinMsg::Resize(c, r) => {
//                         let _ = master.resize(PtySize {
//                             rows: r, cols: c, pixel_width: 0, pixel_height: 0
//                         });
//                     }
//                 }
//             }
//         });
//
//         // ── 5. 进程监控与清理任务 (主控制线) ──────────────────────────────────
//         let cb_main = callbacks.clone();
//         let child_arc = Arc::new(std::sync::Mutex::new(child));
//         let child_clone = child_arc.clone();
//
//         let join = tokio::spawn(async move {
//             let mut exit_code: Option<i32> = None;
//
//             loop {
//                 tokio::select! {
//                     _ = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
//                         let mut c = child_clone.lock().unwrap();
//                         if let Ok(Some(status)) = c.try_wait() {
//                             exit_code = Some(status.exit_code() as i32);
//                             break;
//                         }
//                     }
//                     _ = &mut drop_rx => {
//                         let mut c = child_clone.lock().unwrap();
//                         let _ = c.kill();
//                         exit_code = c.try_wait().ok().flatten().map(|s| s.exit_code() as i32);
//                         break;
//                     }
//                 }
//             }
//
//             let fut_opt = {
//                 let mut cb = cb_main.lock().await;
//                 cb.on_exit.as_mut().map(|f| f(exit_code))
//             };
//             if let Some(fut) = fut_opt { fut.await; }
//
//             let _ = tokio::join!(out_task, stdin_task);
//
//             let fut_opt = {
//                 let mut cb = cb_main.lock().await;
//                 cb.on_close.as_mut().map(|f| f())
//             };
//             if let Some(fut) = fut_opt { fut.await; }
//         });
//
//         Ok((tx_stdin, drop_tx, join))
//     }
// }
//
// impl Drop for PtyShell {
//     fn drop(&mut self) {
//         let _ = self.close();
//     }
// }
//
// fn build_command_builder(shell: &str, shell_name: &str) -> Result<CommandBuilder> {
//     let mut cmd = CommandBuilder::new(shell);
//
//     if shell_name == ss!("bash") {
//         cmd.args([s!("--norc"), s!("--noprofile"), s!("-i")]);
//     } else if shell_name == ss!("zsh") {
//         cmd.args([s!("-f"), s!("-i")]);
//     } else if shell_name == ss!("sh") {
//         cmd.arg(s!("-i"));
//     } else if shell_name == ss!("fish") {
//         cmd.args([s!("--no-config"), s!("-i")]);
//     } else if shell_name == ss!("cmd") {
//         cmd.args([s!("/Q"), s!("/K"), s!("prompt $G")]);
//     } else if shell_name == ss!("powershell") || shell_name == ss!("pwsh") {
//         cmd.args([
//             s!("-ep"),
//             s!("Bypass"),
//         ]);
//     } else if shell_name == ss!("python") {
//         cmd.args([s!("-u"), s!("-i")]);
//         cmd.env("PYTHONUTF8", "1");
//     } else if shell_name == ss!("node") {
//         cmd.arg(s!("-i"));
//     } else {
//         anyhow::bail!("{}{shell_name}",s!("unsupported shell: "));
//     }
//     cmd.env("TERM", "xterm-256color");
//
//     Ok(cmd)
// }
//
// fn init_command(shell_name: &str) -> Option<String> {
//     if shell_name == ss!("cmd") {
//         Some(s!("chcp 65001 >nul 2>&1\r\n"))
//     } else if shell_name == ss!("powershell") || shell_name == ss!("pwsh") {
//         Some(s!(
//             "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
//              [Console]::InputEncoding  = [System.Text.Encoding]::UTF8; \
//              $OutputEncoding           = [System.Text.Encoding]::UTF8\n"
//         ))
//     } else {
//         None
//     }
// }
//
// fn exit_command(shell_path: &str) -> String {
//     let shell_name = normalize_shell_name(shell_path).unwrap_or_default();
//     let name_str = shell_name.as_str();
//     if name_str == ss!("python") {
//         s!("quit()\r")
//     } else if name_str == ss!("node") {
//         s!(".exit\r")
//     } else if name_str == ss!("powershell") || name_str == ss!("pwsh") {
//         s!("exit\r")
//     } else {
//         s!("exit\r")
//     }
// }
//
// #[cfg(test)]
// mod tests {
//     use std::time::Duration;
//     use super::*;
//     #[tokio::test]
//     async fn test_pty_shell() {
//         let mut shell = PtyShell::new("bash")
//             .on_output(async move |s| print!("{s}"))
//             .on_exit(async move |code| eprintln!("exit: {:?}", code))
//             .on_close(async move || println!("[close]"))
//             .spawn()
//             .await
//             .unwrap();
//         shell.send_line("ls").await.unwrap();
//         shell.send_line("pwd").await.unwrap();
//         tokio::time::sleep(Duration::from_millis(500)).await;
//         shell.close().unwrap();
//         shell.join().await.unwrap();
//
//     }
// }