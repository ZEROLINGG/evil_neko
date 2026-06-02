// lib/src/sandbox/mod.rs
#![cfg(feature = "sandbox")]
#![allow(unused)]

pub mod fs;
pub mod env;

use std::collections::{HashMap, HashSet};
use std::fmt;
use anyhow::{anyhow, ensure, Result};
use serde::{Deserialize, Serialize};
use crate::{s, ss, s_fmt}; // 字符串加密宏



#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub enum ScoreType {
    Env, File, Directory, FileContent, Process, Service,
    Driver, Registry, Cpu, Dmi, Bios, Network, UserActivity,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ScoreEntry {
    pub msg: String,
    pub score: u8,       // 0 到 10，表示单条证据的绝对重要程度
    pub confidence: f32, // 0.0 到 1.0，表示对这条证据的置信度
}

impl ScoreEntry {
    // 单条证据的有效得分 (0.0 - 10.0)
    pub fn effective_score(&self) -> f32 {
        self.score as f32 * self.confidence
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Score {
    entries: HashMap<ScoreType, Vec<ScoreEntry>>,
    weight: HashMap<ScoreType, u8>, // 权重系统，0到10
}

impl Default for Score {
    fn default() -> Self {
        let mut default_weights = HashMap::new();
        default_weights.insert(ScoreType::Bios, 10);
        default_weights.insert(ScoreType::Dmi, 10);
        default_weights.insert(ScoreType::Cpu, 9);
        default_weights.insert(ScoreType::Driver, 9);

        default_weights.insert(ScoreType::Service, 8);
        default_weights.insert(ScoreType::Process, 8);
        default_weights.insert(ScoreType::Registry, 7);

        default_weights.insert(ScoreType::FileContent, 6);
        default_weights.insert(ScoreType::Network, 5);
        default_weights.insert(ScoreType::UserActivity, 5);

        default_weights.insert(ScoreType::File, 4);
        default_weights.insert(ScoreType::Directory, 3);
        default_weights.insert(ScoreType::Env, 2);

        Self {
            entries: HashMap::new(),
            weight: default_weights,
        }
    }
}

impl Score {
    /// 接受自定义权重字典。如果传入 None，则使用内置的默认反沙箱策略权重。
    pub fn new(custom_weights: Option<HashMap<ScoreType, u8>>) -> Self {
        match custom_weights {
            Some(weights) => {
                let sanitized_weights = weights
                    .into_iter()
                    .map(|(k, v)| (k, v.min(10)))
                    .collect();
                Self {
                    entries: HashMap::new(),
                    weight: sanitized_weights,
                }
            }
            None => Self::default(),
        }
    }

    pub fn add<S: Into<String>>(&mut self, typ: ScoreType, msg: S, score: u8, confidence: f32) {
        let entry = ScoreEntry {
            msg: msg.into(),
            score: score.min(10),
            confidence: confidence.clamp(0.0, 1.0),
        };
        self.entries.entry(typ).or_default().push(entry);
    }

    /// 公式: 1 - ∏(1 - (score_i / 10) * (weight_i / 10))，最终映射到 0-100。
    pub fn calculate_score(&self, typ: Option<ScoreType>) -> f32 {
        let mut fail_prob = 1.0_f32;

        let mut process_entry = |t: ScoreType, entry: &ScoreEntry, current_prob: &mut f32| {
            let w = self.weight.get(&t).cloned().unwrap_or(5) as f32 / 10.0;
            let base_hit_prob = entry.effective_score() / 10.0;

            let final_hit_prob = base_hit_prob * w;

            *current_prob *= 1.0 - final_hit_prob;
        };

        match typ {
            Some(t) => {
                if let Some(list) = self.entries.get(&t) {
                    for entry in list {
                        process_entry(t, entry, &mut fail_prob);
                    }
                }
            }
            None => {
                for (&t, list) in &self.entries {
                    for entry in list {
                        process_entry(t, entry, &mut fail_prob);
                    }
                }
            }
        }

        // 最终得分，最大无限趋近于 100
        let final_score = (1.0 - fail_prob) * 100.0;

        final_score.min(100.0).max(0.0)
    }

    pub fn get_entries(&self) -> &HashMap<ScoreType, Vec<ScoreEntry>> {
        &self.entries
    }

    pub fn get_weights(&self) -> &HashMap<ScoreType, u8> {
        &self.weight
    }
}


#[derive(Clone, Serialize, Deserialize)]
pub struct EvidenceCollection<T: std::hash::Hash + Eq> {
    pub evidence: HashMap<T, Score>,
    target_weights: HashMap<T, u8>,
    score_weights: Option<HashMap<ScoreType, u8>>,
}
impl<T: std::hash::Hash + Eq> Default for EvidenceCollection<T> {
    fn default() -> Self {
        Self {
            evidence: HashMap::new(),
            target_weights: HashMap::new(),
            score_weights: None,
        }
    }
}

impl<T: std::hash::Hash + Eq + Clone> EvidenceCollection<T> {
    /// 接受针对目标 T 的自定义权重字典。如果传入 None，则默认所有目标的权重都是 10 (不缩放)。
    pub fn new(custom_target_weights: Option<HashMap<T, u8>>, custom_score_weights: Option<HashMap<ScoreType, u8>>) -> Self {
        let weights = custom_target_weights
            .unwrap_or_default()
            .into_iter()
            .map(|(k, v)| (k, v.min(10)))
            .collect();

        Self {
            evidence: HashMap::new(),
            target_weights: weights,
            score_weights: custom_score_weights,
        }
    }

    pub fn add<S: Into<String>>(&mut self, key: T, typ: ScoreType, msg: S, score: u8, confidence: f32) {
        self.evidence
            .entry(key)
            .or_insert_with(|| Score::new(self.score_weights.clone()))
            .add(typ, msg, score, confidence);
    }

    pub fn get_score_for(&self, key: &T) -> f32 {
        if let Some(score_obj) = self.evidence.get(key) {
            let base_score = score_obj.calculate_score(None);
            let t_weight = self.target_weights.get(key).cloned().unwrap_or(10) as f32 / 10.0;
            (base_score * t_weight).min(100.0).max(0.0)
        } else {
            0.0
        }
    }
    pub fn score(&self) -> f32 {
        let mut safe_prob = 1.0_f32;

        for key in self.evidence.keys() {
            let target_score = self.get_score_for(key);
            let hit_prob = target_score / 100.0;

            safe_prob *= 1.0 - hit_prob;
        }
        let final_score = (1.0 - safe_prob) * 100.0;
        final_score.clamp(0.0, 100.0)
    }

    pub fn is_empty(&self) -> bool {
        self.evidence.is_empty()
    }

    pub fn get_target_weights(&self) -> &HashMap<T, u8> {
        &self.target_weights
    }
}



// 定义各种检测目标类型
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SandboxType { #[default]
Cuckoo, CAPE, Zenbox, JoeSandbox, Unknown }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum EmulatorType { #[default]
Bochs, QemuTCG, Unicorn, Unknown }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum VirtualMachineType { #[default]
VMware, VirtualBox, HyperV, Xen, KVM, Parallels, Unknown }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ContainerType { #[default]
Docker, Podman, LXC, Containerd, Kubernetes, Wsl, Unknown }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SoftwareType { #[default]
Analysis, Debugger, Security }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TrustType {
    #[default]
    PersonalFiles,
    Browser,
    InstalledSoftware,
    UserAccounts,
    SystemUptime,
    FileModificationTime,
    RegistryUsage,
    EventLogs,
    PhysicalDevices,
    BiosAge,
    Network,
    EmailClient,
    CloudSync,
    Development,
    Game,
}


#[derive(Default, Serialize, Deserialize)]
pub struct Environment {
    pub sandbox: EvidenceCollection<SandboxType>,
    pub virtual_machine: EvidenceCollection<VirtualMachineType>,
    pub emulator: EvidenceCollection<EmulatorType>,
    pub container: EvidenceCollection<ContainerType>,
    pub software: EvidenceCollection<SoftwareType>,

    pub trust: EvidenceCollection<TrustType>,

    pub risk_weight: f32,  // 默认 1.0
    pub trust_weight: f32, // 默认 0.6
}

impl Environment {
    pub fn new() -> Self {
        let mut sandbox_weights = HashMap::new();
        sandbox_weights.insert(SandboxType::CAPE, 10);
        sandbox_weights.insert(SandboxType::Cuckoo, 9);
        sandbox_weights.insert(SandboxType::JoeSandbox, 9);
        sandbox_weights.insert(SandboxType::Zenbox, 8);
        sandbox_weights.insert(SandboxType::Unknown, 4);

        let mut emulator_weights = HashMap::new();
        emulator_weights.insert(EmulatorType::QemuTCG, 8);
        emulator_weights.insert(EmulatorType::Bochs, 8);
        emulator_weights.insert(EmulatorType::Unicorn, 6);
        emulator_weights.insert(EmulatorType::Unknown, 4);

        let mut vm_weights = HashMap::new();
        vm_weights.insert(VirtualMachineType::VMware, 8);
        vm_weights.insert(VirtualMachineType::VirtualBox, 8);
        vm_weights.insert(VirtualMachineType::HyperV, 6);
        vm_weights.insert(VirtualMachineType::Unknown, 4);

        let mut container_weights = HashMap::new();
        container_weights.insert(ContainerType::Docker, 5);
        container_weights.insert(ContainerType::Wsl, 2);
        container_weights.insert(ContainerType::Unknown, 1);

        let mut sw_weights = HashMap::new();
        sw_weights.insert(SoftwareType::Debugger, 10);        // 发现调试器直接最高威胁
        sw_weights.insert(SoftwareType::Analysis, 5);
        sw_weights.insert(SoftwareType::Security, 4);
        let mut trust_weights = HashMap::new();


        trust_weights.insert(TrustType::PhysicalDevices, 10);
        trust_weights.insert(TrustType::Game, 10);

        trust_weights.insert(TrustType::CloudSync, 9);     // OneDrive/Dropbox 登录态
        trust_weights.insert(TrustType::EmailClient, 9);   // 邮件客户端及本地数据库
        trust_weights.insert(TrustType::Browser, 8); // 复杂的浏览器历史、Cookies
        trust_weights.insert(TrustType::Network, 8); // 丰富的已知Wi-Fi列表/内网ARP

        trust_weights.insert(TrustType::InstalledSoftware, 7); // 微信、QQ、钉钉等
        trust_weights.insert(TrustType::UserAccounts, 6);      // 非Admin/默认的真实用户、微软账号绑定
        trust_weights.insert(TrustType::EventLogs, 6);         // 庞大且连续的 Windows 事件日志

        trust_weights.insert(TrustType::RegistryUsage, 5);     // 注册表体积（膨胀度）
        trust_weights.insert(TrustType::SystemUptime, 4);      // 开机时间长（极易被欺骗）
        trust_weights.insert(TrustType::BiosAge, 4);           // BIOS老旧（极易在VM中配置）
        trust_weights.insert(TrustType::PersonalFiles, 5);     // 个人文档目录
        trust_weights.insert(TrustType::FileModificationTime, 3); // 文件的散乱修改时间

        let mut trust_score_weights = HashMap::new();
        trust_score_weights.insert(ScoreType::UserActivity, 10);
        trust_score_weights.insert(ScoreType::FileContent, 8);
        trust_score_weights.insert(ScoreType::Process, 8);
        trust_score_weights.insert(ScoreType::Directory, 8);
        trust_score_weights.insert(ScoreType::Registry, 7);
        trust_score_weights.insert(ScoreType::Network, 6);
        trust_score_weights.insert(ScoreType::File, 5);
        trust_score_weights.insert(ScoreType::Service, 4);
        trust_score_weights.insert(ScoreType::Env, 5);

        trust_score_weights.insert(ScoreType::Driver, 2);
        trust_score_weights.insert(ScoreType::Cpu, 1);
        trust_score_weights.insert(ScoreType::Dmi, 1);
        trust_score_weights.insert(ScoreType::Bios, 1);

        Self {
            sandbox: EvidenceCollection::new(Some(sandbox_weights), None),
            virtual_machine: EvidenceCollection::new(Some(vm_weights), None),
            emulator: EvidenceCollection::new(Some(emulator_weights), None),
            container: EvidenceCollection::new(Some(container_weights), None),
            software: EvidenceCollection::new(Some(sw_weights), None),

            // 注入为 Trust 量身定做的两套权重
            trust: EvidenceCollection::new(Some(trust_weights), Some(trust_score_weights)),

            risk_weight: 1.0,
            trust_weight: 0.45,
        }
    }

    /// 获取纯粹的风险分数 (0.0 - 100.0)
    pub fn base_risk_score(&self) -> f32 {
        let mut global_safe_prob = 1.0_f32;
        let dimensions = [
            self.sandbox.score(),
            self.virtual_machine.score(),
            self.emulator.score(),
            self.container.score(),
            self.software.score(),
        ];

        for score in dimensions {
            global_safe_prob *= 1.0 - (score / 100.0);
        }
        ((1.0 - global_safe_prob) * 100.0).clamp(0.0, 100.0)
    }

    /// 获取纯粹的可信度分数 (0.0 - 100.0)
    pub fn trust_score(&self) -> f32 {
        self.trust.score()
    }
    pub fn final_risk_score(&self) -> f32 {
        let risk = self.base_risk_score() * self.risk_weight;
        let trust = self.trust_score();

        if risk <= 0.0 { return 0.0; }

        let risk_ratio = risk / 100.0;


        let lambda = 3.0_f32;
        let mitigation_efficiency = (-lambda * risk_ratio).exp();

        let trust_mitigation = trust * self.trust_weight * mitigation_efficiency;

        let final_score = risk - trust_mitigation;
        final_score.clamp(0.0, 100.0)
    }


    pub fn should_abort(&self, threshold: f32) -> bool {
        self.final_risk_score() >= threshold
    }

    pub fn sandbox<S: Into<String>>(&mut self, key: SandboxType, typ: ScoreType, msg: S, score: u8, confidence: f32) {
        self.sandbox.add(key, typ, msg, score, confidence);
    }

    pub fn virtual_machine<S: Into<String>>(&mut self, key: VirtualMachineType, typ: ScoreType, msg: S, score: u8, confidence: f32) {
        self.virtual_machine.add(key, typ, msg, score, confidence);
    }

    pub fn emulator<S: Into<String>>(&mut self, key: EmulatorType, typ: ScoreType, msg: S, score: u8, confidence: f32) {
        self.emulator.add(key, typ, msg, score, confidence);
    }

    pub fn container<S: Into<String>>(&mut self, key: ContainerType, typ: ScoreType, msg: S, score: u8, confidence: f32) {
        self.container.add(key, typ, msg, score, confidence);
    }

    pub fn software<S: Into<String>>(&mut self, key: SoftwareType, typ: ScoreType, msg: S, score: u8, confidence: f32) {
        self.software.add(key, typ, msg, score, confidence);
    }

    // 快捷添加可信证据的方法
    pub fn trust<S: Into<String>>(&mut self, key: TrustType, typ: ScoreType, msg: S, score: u8, confidence: f32) {
        self.trust.add(key, typ, msg, score, confidence);
    }



    /// 生成格式化的环境审计报告（包含总览、风险分布、纯净过滤与严格缩进）
    pub fn dump_report(&self) -> String {
        use std::fmt::Write;
        let mut report = String::new();

        // ==========================================
        // 1. 头部与总览层 (Level 0 & Level 1)
        // ==========================================
        let _ = writeln!(&mut report, "======================================================================");
        let _ = writeln!(&mut report, "                       ENVIRONMENT INSPECTION REPORT                  ");
        let _ = writeln!(&mut report, "======================================================================");
        let _ = writeln!(&mut report, "[OVERVIEW]");
        let _ = writeln!(&mut report, "  - Final Risk Score     : {:.2}%", self.final_risk_score());
        let _ = writeln!(&mut report, "  - Base Risk Score      : {:.2}%", self.base_risk_score());
        let _ = writeln!(&mut report, "  - Trust Score          : {:.2}%", self.trust_score());
        let _ = writeln!(&mut report, "  - Configuration Weights: Risk {:.2} / Trust {:.2}",
                         self.risk_weight, self.trust_weight
        );
        let _ = writeln!(&mut report, "  - Verdict              : {}",
                         if self.should_abort(50.0) { "⚠️  ABORTED (High Risk)" } else { "✅  PASSED" }
        );
        let _ = writeln!(&mut report);

        // ==========================================
        // 2. 详情层 (通过闭包手动映射未实现 Debug 的枚举)
        // ==========================================
        let _ = writeln!(&mut report, "[DETAILED BREAKDOWN]");

        let mut has_details = false;

        // 审计维度: 沙箱
        Self::format_dimension(
            &mut report,
            "Sandbox Detections",
            &self.sandbox,
            |t| match t {
                SandboxType::Cuckoo => "Cuckoo",
                SandboxType::CAPE => "CAPE",
                SandboxType::Zenbox => "Zenbox",
                SandboxType::JoeSandbox => "JoeSandbox",
                SandboxType::Unknown => "Unknown Sandbox Environment",
            },
            &mut has_details
        );

        // 审计维度: 虚拟机
        Self::format_dimension(
            &mut report,
            "Virtual Machine Detections",
            &self.virtual_machine,
            |t| match t {
                VirtualMachineType::VMware => "VMware",
                VirtualMachineType::VirtualBox => "VirtualBox",
                VirtualMachineType::HyperV => "HyperV",
                VirtualMachineType::Xen => "Xen",
                VirtualMachineType::KVM => "KVM",
                VirtualMachineType::Parallels => "Parallels",
                VirtualMachineType::Unknown => "Unknown VM Hypervisor",
            },
            &mut has_details
        );

        // 审计维度: 模拟器
        Self::format_dimension(
            &mut report,
            "Emulator Detections",
            &self.emulator,
            |t| match t {
                EmulatorType::Bochs => "Bochs",
                EmulatorType::QemuTCG => "Qemu TCG",
                EmulatorType::Unicorn => "Unicorn Engine",
                EmulatorType::Unknown => "Unknown Emulator",
            },
            &mut has_details
        );

        // 审计维度: 容器
        Self::format_dimension(
            &mut report,
            "Container Detections",
            &self.container,
            |t| match t {
                ContainerType::Docker => "Docker",
                ContainerType::Podman => "Podman",
                ContainerType::LXC => "LXC",
                ContainerType::Containerd => "Containerd",
                ContainerType::Kubernetes => "Kubernetes Node",
                ContainerType::Wsl => "WSL Subsystem",
                ContainerType::Unknown => "Unknown Container Runtime",
            },
            &mut has_details
        );

        // 审计维度: 调试/安全软件
        Self::format_dimension(
            &mut report,
            "Analysis & Security Software",
            &self.software,
            |t| match t {
                SoftwareType::Analysis => "Analysis Tools",
                SoftwareType::Debugger => "Debugger/Reverse Engineering Tools",
                SoftwareType::Security => "Security/Antivirus Products",
            },
            &mut has_details
        );

        // 审计维度: 可信度画像 (正面证据)
        Self::format_dimension(
            &mut report,
            "Trust Mitigation Indicators",
            &self.trust,
            |t| match t {
                TrustType::PersonalFiles => "User Personal Files Profile",
                TrustType::Browser => "Browser History & Cookies Depth",
                TrustType::InstalledSoftware => "Daily IM/Working Software Footprints",
                TrustType::UserAccounts => "Authenticated Non-Default User Accounts",
                TrustType::SystemUptime => "Natural System Uptime Continuity",
                TrustType::FileModificationTime => "Scattered File Timestamps Distribution",
                TrustType::RegistryUsage => "Natural Registry Volume Bloat",
                TrustType::EventLogs => "Continuous Windows Event Logs Storage",
                TrustType::PhysicalDevices => "Physical Hardware/Peripheral Count",
                TrustType::BiosAge => "Legitimate BIOS Age Validity",
                TrustType::Network => "Rich WiFi/ARP Network Association History",
                TrustType::EmailClient => "Local Active Email Databases",
                TrustType::CloudSync => "Active Cloud Sync Sessions (OneDrive/Dropbox)",
                TrustType::Development => "Heavy Developer IDE/Git Environment Traces",
                TrustType::Game => "PC Gaming Platform Footprints (Steam etc.)",
            },
            &mut has_details
        );

        if !has_details {
            let _ = writeln!(&mut report, "  (No clean evidence logs or context anomalies were collected)");
        }

        let _ = writeln!(&mut report, "======================================================================");
        report
    }

    /// 针对单维度集合的内部流式清洗与格式化输出（动态维护缩进与空内容过滤）
    fn format_dimension<T>(
        out: &mut String,
        label: &str,
        collection: &EvidenceCollection<T>,
        name_map: impl Fn(&T) -> &'static str,
        has_details_flag: &mut bool,
    ) where
        T: std::hash::Hash + Eq + Clone,
    {
        if collection.is_empty() {
            return;
        }

        use std::fmt::Write;
        *has_details_flag = true;

        // Level 1 缩进: 2 空格 (大维度分类)
        let _ = writeln!(out, "  - {} (Dimension Combined Score: {:.2}/100)", label, collection.score());

        for (target, score_obj) in &collection.evidence {
            let target_name = name_map(target);
            let target_score = collection.get_score_for(target);

            // Level 2 缩进: 4 空格 (具体目标，如 CAPE 或 Docker)
            let _ = writeln!(out, "    * Target: {} (Weighted Contribution: {:.2}/100)", target_name, target_score);

            for (score_type, entries) in score_obj.get_entries() {
                if entries.is_empty() {
                    continue;
                }

                // Level 3 缩进: 6 空格 (底层证据行为类别，如 Registry 或 FileContent)
                let _ = writeln!(out, "      + [{:?}] Layer Evidences:", score_type);

                for entry in entries {
                    // Level 4 缩进: 8 空格 (证据源快照元数据)
                    let _ = writeln!(
                        out,
                        "        [!] Hit: \"{}\" [Base: {}, Conf: {:.2}, Effective: {:.2}]",
                        entry.msg,
                        entry.score,
                        entry.confidence,
                        entry.effective_score()
                    );
                }
            }
        }
        let _ = writeln!(out);
    }
}

