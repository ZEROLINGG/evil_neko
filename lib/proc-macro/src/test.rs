// lib/proc-macro/src/test.rs
#![allow(unused)]
use std::{fs, thread};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};



/// 用于自动管理锁文件生命周期的结构体
struct DynTestLock {
    lock_path: PathBuf,
}

impl DynTestLock {
    fn new(id: &str) -> Self {
        let mut lock_path = std::env::current_dir().unwrap();
        lock_path.push("target");
        lock_path.push("dyn_tests");
        lock_path.push("lock");

        // 确保锁目录存在
        fs::create_dir_all(&lock_path).expect("无法创建锁目录");

        let lock_file = lock_path.join(format!("lock_{}", id));
        fs::File::create(&lock_file).expect("无法创建锁文件");

        Self { lock_path: lock_file }
    }
}

impl Drop for DynTestLock {
    fn drop(&mut self) {
        if self.lock_path.exists() {
            let _ = fs::remove_file(&self.lock_path);
        }
    }
}

/// 清理所有动态测试目录，支持超时等待锁释放
pub fn clear_dny_project(timeout: Option<Duration>) {
    thread::sleep(Duration::from_secs_f32(0.3));
    let mut base_dir = std::env::current_dir().unwrap();
    base_dir.push("target");
    base_dir.push("dyn_tests");

    let lock_dir = base_dir.join("lock");
    let start_time = Instant::now();

    // 检查是否有正在运行的锁
    if lock_dir.exists() {
        loop {
            let has_locks = fs::read_dir(&lock_dir)
                .map(|mut entries| entries.any(|entry| entry.is_ok()))
                .unwrap_or(false);

            if !has_locks {
                break; // 锁文件全没了，安全退出循环
            }

            // 检查是否超时
            if let Some(limit) = timeout {
                if start_time.elapsed() >= limit {
                    panic!("清除动态测试失败：等待锁文件释放超时！");
                }
            }

            // 稍微等待再重试，避免 CPU 狂飙
            thread::sleep(Duration::from_millis(100));
        }
    }

    // 所有锁已释放（或压根没有锁），安全删除整个目录
    if base_dir.exists() {
        fs::remove_dir_all(base_dir).expect("删除 dyn_tests 目录失败");
    }
}

pub(crate) struct Timer(&'static str, Instant);

impl Timer {
    pub(crate) fn new(name: &'static str) -> Self {
        Self(name, Instant::now())
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        eprintln!("{} took {:?}", self.0, self.1.elapsed());
    }
}



#[derive(Debug, Clone)]
pub struct DnyResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub build_duration: Duration,
    pub run_duration: Duration,
    pub ok: bool,
}
impl Default for DnyResult {
    fn default() -> Self {
        Self {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: -1,
            build_duration: Duration::ZERO,
            run_duration: Duration::ZERO,
            ok: false,
        }
    }
}

pub fn dny_run(main_code: &str, deps: &str, timeout: Option<Duration>, only_build: bool) -> DnyResult {
    let dny_runner = DnyRun::new(main_code, deps);
    if only_build { dny_runner.build(timeout) } else { dny_runner.run(timeout) }
}
pub fn dny_run_batch(
    task: Vec<(String, String, Option<String>)>,
    task_timeout: Option<Duration>,
    only_build: Option<bool>,
    on_result: Option<Box<dyn Fn(String, DnyResult)>>,
) -> Vec<(String, DnyResult)> {
    let only_build_flag = only_build.unwrap_or(false);

    let mut results = Vec::with_capacity(task.len());

    for (index, (main_code, deps, tag)) in task.into_iter().enumerate() {
        let tag = tag.unwrap_or_else(|| format!("task_{}", index));

        let res = dny_run(
            &main_code,
            &deps,
            task_timeout,
            only_build_flag,
        );

        if let Some(ref callback) = on_result {
            callback(tag.clone(), res.clone());
        }

        results.push((tag, res));
    }

    results
}

pub struct DnyRun {
    pub main_code: String,
    pub deps: String,
    dir: PathBuf,
    project_name: String,
    lock: DynTestLock,
}

impl DnyRun {
    /// 初始化共享缓存的动态运行环境
    pub fn new(main_code: &str, deps: &str) -> Self {
        let id = format!(
            "{}_{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
            rand::random::<u32>()
        );
        let project_name = format!("dyn_test_{}", id);


        let mut dir = std::env::current_dir().unwrap();
        dir.push("target");
        dir.push("dyn_tests");
        dir.push(format!("dny_{}", id));

        let src_dir = dir.join("src");
        fs::create_dir_all(&src_dir).expect("无法创建共享测试目录");

        let mut instance = Self {
            main_code: main_code.to_string(),
            deps: deps.to_string(),
            dir,
            project_name: project_name.clone(),
            lock: DynTestLock::new(&project_name),
        };

        instance.sync_files();
        instance
    }

    pub fn sync_files(&self) {
        let cargo_toml = format!(
            r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
{}
"#,
            self.project_name, self.deps
        );

        fs::write(self.dir.join("Cargo.toml"), cargo_toml).expect("写入 Cargo.toml 失败");
        fs::write(self.dir.join("src").join("main.rs"), &self.main_code).expect("写入 main.rs 失败");
    }

    fn execute_cmd(cmd: &mut Command, timeout: Option<Duration>) -> Result<(Output, Duration), String> {
        let start_time = Instant::now();

        // 启动子进程
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("进程启动失败: {}", e))?;

        let mut stdout_pipe = child.stdout.take().expect("无法获取 stdout");
        let mut stderr_pipe = child.stderr.take().expect("无法获取 stderr");

        let stdout_handle = thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stdout_pipe.read_to_end(&mut buf);
            buf
        });

        let stderr_handle = thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stderr_pipe.read_to_end(&mut buf);
            buf
        });

        match timeout {
            None => {
                let status = child.wait().map_err(|e| e.to_string())?;
                // 进程正常结束，回收线程读取的数据
                let stdout = stdout_handle.join().unwrap_or_default();
                let stderr = stderr_handle.join().unwrap_or_default();
                Ok((Output { status, stdout, stderr }, start_time.elapsed()))
            }
            Some(limit) => {
                let check_interval = Duration::from_millis(10);
                loop {
                    if let Ok(Some(status)) = child.try_wait() {
                        // 子进程正常结束了，回收数据
                        let stdout = stdout_handle.join().unwrap_or_default();
                        let stderr = stderr_handle.join().unwrap_or_default();
                        return Ok((Output { status, stdout, stderr }, start_time.elapsed()));
                    }

                    if start_time.elapsed() >= limit {
                        let _ = child.kill();
                        return Err(format!("执行超时！超出限制: {:?}", limit));
                    }
                    thread::sleep(check_interval);
                }
            }
        }
    }


    /// 仅构建代码
    pub fn build(&self, timeout: Option<Duration>) -> DnyResult {
        let mut result = DnyResult::default();

        let mut cmd = Command::new("cargo");
        cmd.arg("build").arg("-q").current_dir(&self.dir);

        match Self::execute_cmd(&mut cmd, timeout) {
            Ok((output, duration)) => {
                result.build_duration = duration;
                result.stdout = String::from_utf8_lossy(&output.stdout).to_string();
                result.stderr = String::from_utf8_lossy(&output.stderr).to_string();
                result.exit_code = output.status.code().unwrap_or(-1);
                result.ok = output.status.success();
            }
            Err(err_msg) => {
                result.stderr = err_msg;
                result.exit_code = -2;
            }
        }
        result
    }

    /// 构建并运行
    pub fn run(&self, timeout: Option<Duration>) -> DnyResult {
        let build_result = self.build(timeout);

        if !build_result.ok {
            return build_result;
        }

        self.run_no_build(timeout, Some(build_result))
    }

    pub fn run_no_build(&self, timeout: Option<Duration>, build_result: Option<DnyResult>) -> DnyResult {
        let bin_path = self.dir.join("target").join("debug")
            .join(if cfg!(windows) { format!("{}.exe", &self.project_name) } else { (&self.project_name).clone() });
        let mut cmd = Command::new(&bin_path);

        let build_result = build_result.unwrap_or_default();
        let mut result = DnyResult::default();

        match Self::execute_cmd(&mut cmd, timeout) {
            Ok((output, duration)) => {
                result.run_duration = duration;
                result.stdout = String::from_utf8_lossy(&output.stdout).to_string();
                result.stderr = String::from_utf8_lossy(&output.stderr).to_string();
                result.exit_code = output.status.code().unwrap_or(-1);
                result.ok = output.status.success();
                result.build_duration = build_result.build_duration;
            }
            Err(err_msg) => {
                result.run_duration = Duration::ZERO;
                result.stderr = err_msg;
                result.exit_code = -2;
                result.ok = false;
                result.build_duration = build_result.build_duration;
            }
        }

        result
    }

    pub fn reset_main_code<S: Into<String>>(&mut self, main_code: S) {
        self.main_code = main_code.into();
        self.sync_files()
    }

    pub fn reset_deps<S: Into<String>>(&mut self, deps: S) {
        self.deps = deps.into();
        self.sync_files()
    }

    pub fn clean(&mut self) {
        if self.dir.exists() {
            let _ = fs::remove_dir_all(&self.dir.join("target"));
        }
    }
}

pub fn dny_run_batch_use_cache(
    task: Vec<(String, String, Option<String>)>, // (main_code, deps, tag)
    task_timeout: Option<Duration>,
    only_build: Option<bool>,
    on_result: Option<Box<dyn Fn(String, DnyResult)>>,
) -> Vec<(String, DnyResult)> {
    let only_build_flag = only_build.unwrap_or(false);
    let mut results = Vec::with_capacity(task.len());

    if task.is_empty() {
        return results;
    }
    let callback = |tag,res| {
        if let Some(ref call) = on_result {
            call(tag, res);
        }
    };

    let (_, first_deps, _) = &task[0];
    let mut runner = DnyRun::new("fn main() {}", first_deps);
    let init_res = runner.build(task_timeout);
    callback("init".to_string(), init_res.clone());
    results.push(("init".to_string(), init_res));

    for (index, (main_code, deps, tag)) in task.into_iter().enumerate() {
        let tag = tag.unwrap_or_else(|| format!("task_{}", index));

        runner.reset_deps(deps);
        runner.reset_main_code(main_code);

        let res = if only_build_flag {
            runner.build(task_timeout)
        } else {
            runner.run(task_timeout)
        };

        callback(tag.clone(), res.clone());

        results.push((tag, res));
    }

    results
}