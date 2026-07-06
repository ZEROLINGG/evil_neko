use std::str::FromStr;
use crate::sandbox::*;
use crate::runtime::*;
use std::time::{Duration, Instant, SystemTime};
use regex::Regex;
use tokio::task::JoinHandle;
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;
use crate::action;

static PATTERNS: LazyLock<Patterns> = LazyLock::new(Patterns::new);

struct Patterns {
    x64dbg: Regex, // 含x32dbg
    windbg: Regex,
    ollydbg: Regex,
    immunity: Regex,

    gdb: Regex,
    lldb: Regex,
    rr: Regex,

    ida: Regex, // 含ida64
    ghidra: Regex,
    radare2: Regex,

    processhacker: Regex,
    cheatengine: Regex,
    artmoney: Regex,
    squalr: Regex,

    procmon: Regex,
}

impl Patterns {
    fn new() -> Self {
        Self {
            // Windows 调试器
            // 匹配 x32dbg, x64dbg, x96dbg
            x64dbg: Regex::new(ss!(r"(?i)^x(32|64|96)dbg")).unwrap(),
            // 匹配 windbg, windbgx (新版预览)
            windbg: Regex::new(ss!(r"(?i)^windbg(x)?")).unwrap(),
            // 匹配 ollydbg, odbg
            ollydbg: Regex::new(ss!(r"(?i)^(ollydbg|odbg)")).unwrap(),
            // 匹配 immunity debugger
            immunity: Regex::new(ss!(r"(?i)^immunity")).unwrap(),

            // Unix / 跨平台调试器
            // 严格匹配 gdb, gdbserver，防止误杀带 gdb 的其他程序 (如 gdbus)
            gdb: Regex::new(ss!(r"(?i)^gdb(server)?(\.exe)?$")).unwrap(),
            // 严格匹配 lldb, lldb-server
            lldb: Regex::new(ss!(r"(?i)^lldb(-server)?(\.exe)?$")).unwrap(),
            // rr (Mozilla 的反向执行调试器)，名称极短，必须使用严格锚点
            rr: Regex::new(ss!(r"(?i)^rr(\.exe)?$")).unwrap(),

            // 反汇编 / 反编译工具
            // 匹配 ida.exe, ida64.exe, idaq.exe, idaq64.exe (防止误杀 midas 等)
            ida: Regex::new(ss!(r"(?i)^ida(q)?(32|64)?(\.exe)?$")).unwrap(),
            ghidra: Regex::new(ss!(r"(?i)^ghidra")).unwrap(),
            // 匹配 radare2 或 r2
            radare2: Regex::new(ss!(r"(?i)^(radare2|r2)(\.exe)?$")).unwrap(),

            // 内存编辑 / 进程分析工具
            // 匹配 ProcessHacker, process hacker, 以及新名字 systeminformer
            processhacker: Regex::new(ss!(r"(?i)^(process\s*hacker|system\s*informer)")).unwrap(),
            // 匹配 CheatEngine, cheat engine, cheatengine-x86_64 等
            cheatengine: Regex::new(ss!(r"(?i)^cheat\s*engine")).unwrap(),
            artmoney: Regex::new(ss!(r"(?i)^artmoney")).unwrap(),
            squalr: Regex::new(ss!(r"(?i)^squalr")).unwrap(),

            // Sysinternals 工具
            // 匹配 procmon.exe, procmon64.exe
            procmon: Regex::new(ss!(r"(?i)^procmon(64)?a?")).unwrap(),
        }
    }
}

#[inline]
fn read_tsc() -> u64 {
    #[cfg(target_arch = "x86_64")]
    unsafe { core::arch::x86_64::_rdtsc() }

    #[cfg(target_arch = "x86")]
    unsafe { core::arch::x86::_rdtsc() }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    { 0 }
}

pub struct TimingChecker {
    main_start_instant: Instant,
    main_start_system: SystemTime,
    main_start_tsc: u64,
    tasks: (
        JoinHandle<(Duration, u32, u128)>,
        JoinHandle<(Duration, u32, u128)>,
        JoinHandle<(Duration, u32, u128)>,
    ),
    z1: u64,
    z2: u64,
}

impl TimingChecker {
    #[inline(never)]
    pub fn new(user_ms_str: Option<&str>) -> Self {
        // 同时记录三种时间
        let main_start_tsc = read_tsc();
        let main_start_instant = Instant::now();
        let main_start_system = SystemTime::now();

        let user_ms = u64::from_str(user_ms_str.unwrap_or("0")).unwrap_or(0);
        let z1 = user_ms + u64::from_str(ss!("350")).unwrap();
        let z2 = user_ms + u64::from_str(ss!("1")).unwrap();

        // 计算密集型闭包
        let run_task = || async move {
            let start = Instant::now();
            let x = ((seed() % 2) + 1) as u8;
            let mut y = seed();
            let mut executed = 0u32;

            for _ in 0..x {
                let mut s1 = seed() as u8;
                let mut s2 = seed() as u8;
                let mut s3 = seed() as u8;
                let mut img = image::ImageBuffer::new(150, 90);
                for (_, _, pixel) in img.enumerate_pixels_mut() {
                    s1 = next_u8(s1);
                    s2 = next_u8(s2);
                    s3 = next_u8(s3);
                    *pixel = image::Rgb([s1, s2, s3]);
                }
                let img: image::DynamicImage = img.into();
                match utils::fs::___tlsh_image(img).await {
                    None => {}
                    Some(h) => {
                        let hash_str = String::from_utf8_lossy(&h.hash()).into_owned();
                        y = derive_u128(hash_str) ^ next_u128(y);
                    }
                }
                executed += 1;
            }
            let elapsed = start.elapsed();
            let avg = if executed > 0 { elapsed / executed } else { elapsed };
            (avg, executed, y)
        };

        let t1 = tokio::spawn(run_task());
        let t2 = tokio::spawn(run_task());
        let t3 = tokio::spawn(run_task());

        Self {
            main_start_instant,
            main_start_system,
            main_start_tsc,
            tasks: (t1, t2, t3),
            z1,
            z2,
        }
    }

    /// 结束计时并进行安全检查
    #[inline(always)]
    pub async fn finish(self) -> bool {
        let elapsed_instant = self.main_start_instant.elapsed();
        let elapsed_tsc = read_tsc().saturating_sub(self.main_start_tsc);

        let elapsed_system = match self.main_start_system.elapsed() {
            Ok(dur) => dur,
            Err(_) => return true,
        };

        let time_diff = if elapsed_instant > elapsed_system {
            elapsed_instant - elapsed_system
        } else {
            elapsed_system - elapsed_instant
        };

        if time_diff.as_millis() > 50 + (seed() % 20) {
            return true; // 发现时钟不一致，疑似被沙箱或调试工具 Hook
        }

        let elapsed_micros = elapsed_instant.as_micros() as u64;
        if self.main_start_tsc != 0 && elapsed_micros > 0 {
            // TSC(周期数) / 微秒 = 兆赫兹 (MHz)
            let cpu_freq_mhz = elapsed_tsc / elapsed_micros;

            // 现代 CPU 的频率通常在 800 MHz 到 6000 MHz 之间
            if cpu_freq_mhz < 400 || cpu_freq_mhz > 12000 {
                return true;
            }
        }

        // 检查子任务结果
        let (res1, res2, res3) = match (self.tasks.0.await, self.tasks.1.await, self.tasks.2.await) {
            (Ok(a), Ok(b), Ok(c)) => (a, b, c),
            _ => return true,
        };

        let base_avg = (res1.0 + res2.0 + res3.0) / 3;
        let y = (res1.2 ^ res2.2 ^ res3.2) / 3;
        let x = (res1.1 + res2.1 + res3.1) / 3;
        println!("{:?}",base_avg);

        let total_elapsed = base_avg + elapsed_instant;

        if y > x as u128 && y ^ x as u128 != seed() {
            if total_elapsed >= Duration::from_millis(self.z1 + (seed() % 100) as u64) {
                true // 耗时过长 -> 疑似单步调试
            }
            else if Duration::from_millis(self.z2 + (seed() % 1) as u64) >= total_elapsed {
                true // 耗时极短 -> 疑似时间加速
            }
            else {
                false // 正常区间
            }
        } else {
            false
        }
    }
}


async fn check_tool_patterns(
    proc_name: &str,
    patterns: &Patterns,
    env: &Arc<Mutex<Environment>>,
    is_parent: bool,
) {
    macro_rules! check_tool {
        ($regex:expr, $tool_type:expr, $base_score:expr) => {
            if $regex.is_match(proc_name) {
                let score = if is_parent { 9 } else { $base_score };
                let confidence = if is_parent { 0.85 } else { 0.7 };

                let msg = if is_parent {
                    s_add!("Launched by debugger/analysis tool: ", proc_name, " (Matched: ", $regex.as_str(), ")")
                } else {
                    s_add!("Analysis tool detected in memory: ", proc_name, " (Matched: ", $regex.as_str(), ")")
                };

                env.lock().await.add(action!(
                    $tool_type,
                    ScoreType::Process,
                    msg,
                    score,
                    confidence
                ));
                // return;
            }
        };
    }

    // --- 调试器 ---
    check_tool!(patterns.x64dbg, SoftwareType::Debugger, 9);
    check_tool!(patterns.windbg, SoftwareType::Debugger, 9);
    check_tool!(patterns.ollydbg, SoftwareType::Debugger, 9);
    check_tool!(patterns.immunity, SoftwareType::Debugger, 9);
    check_tool!(patterns.gdb, SoftwareType::Debugger, 8);
    check_tool!(patterns.lldb, SoftwareType::Debugger, 8);
    check_tool!(patterns.rr, SoftwareType::Debugger, 8);

    // --- 逆向/反编译工具 ---
    check_tool!(patterns.ida, SoftwareType::Analysis, 8);
    check_tool!(patterns.ghidra, SoftwareType::Analysis, 8);
    check_tool!(patterns.radare2, SoftwareType::Analysis, 8);

    // --- 内存编辑/监控 ---
    check_tool!(patterns.processhacker, SoftwareType::Debugger, 7);
    check_tool!(patterns.cheatengine, SoftwareType::Debugger, 8);
    check_tool!(patterns.artmoney, SoftwareType::Debugger, 6);
    check_tool!(patterns.squalr, SoftwareType::Debugger, 6);

    // --- 系统监控 ---
    check_tool!(patterns.procmon, SoftwareType::Analysis, 7);
}

/// 导出给外部调用的主检测函数
pub async fn check_debugger(env: Arc<Mutex<Environment>>) {
    let timing_checker = TimingChecker::new(Some("2"));

    // 直接借用 LazyLock 获取实例引用，免去 async 开销
    let patterns = &*PATTERNS;

    if let Some(parent_process) = crate::utils::get_parent_process() {
        let parent_lower = parent_process.to_lowercase();
        check_tool_patterns(&parent_lower, patterns, &env, true).await;
    }

    let processes = crate::utils::get_running_processes();
    for proc in processes {
        check_tool_patterns(&proc, patterns, &env, false).await;
    }


    if timing_checker.finish().await {
        env.lock().await.add(action!(
            AbnormalType::Inconsistent,
            ScoreType::OtherSystemApi,
            s!("Execution timing anomaly detected (Possible Single-Step Debugging, Sandbox Time Acceleration, or TSC Hooking)"),
            9,
            0.7
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_normal_execution() {
        let checker = TimingChecker::new(Some("1"));
        let _ = seed();
        let is_suspicious = checker.finish().await;
        assert!(!is_suspicious, "Normal execution should not trigger");
    }

    #[tokio::test]
    async fn test_debugger_module() {
        let env = Environment::new();
        check_debugger(env.clone()).await;
        println!("{}", env.lock().await.dump_report());
    }
}