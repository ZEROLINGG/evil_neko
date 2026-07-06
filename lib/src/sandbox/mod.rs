// lib/src/sandbox/mod.rs
#![cfg(feature = "sandbox")]

pub mod fs;
pub mod env;
pub mod utils;
pub mod general;
pub mod emulator;
pub mod virtual_machine;
pub mod sandbox;


use std::borrow::Cow;
use std::collections::{HashMap};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::runtime::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
pub enum ScoreType {
    Env,
    File, Directory, FileContent,
    Process, Service,
    Driver, Registry,
    Network,
    UserActivity,
    OsBuild,
    StrongFingerprint,
    Gpu, Disk, Cpu, Dmi, Bios,
    OtherSystemApi,
    Other
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ScoreEntry {
    pub msg: String,
    pub score: u8,       // 0 到 10，表示单条证据的绝对重要程度
    pub confidence: f32, // 0.0 到 1.0，表示对这条证据的置信度
}

impl ScoreEntry {
    pub fn effective_score(&self) -> f32 {
        self.score as f32 * self.confidence
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Score {
    entries: HashMap<ScoreType, Vec<ScoreEntry>>,
    weight: HashMap<ScoreType, u8>,
}

impl Default for Score {
    fn default() -> Self {
        let mut default_weights = HashMap::new();
        default_weights.insert(ScoreType::Bios, 8);
        default_weights.insert(ScoreType::Dmi, 8);
        default_weights.insert(ScoreType::Cpu, 7);
        default_weights.insert(ScoreType::Driver, 8);
        default_weights.insert(ScoreType::Service, 9);
        default_weights.insert(ScoreType::Process, 10);
        default_weights.insert(ScoreType::Registry, 7);
        default_weights.insert(ScoreType::FileContent, 4);
        default_weights.insert(ScoreType::Network, 6);
        default_weights.insert(ScoreType::UserActivity, 4);
        default_weights.insert(ScoreType::File, 3);
        default_weights.insert(ScoreType::Directory, 3);
        default_weights.insert(ScoreType::Env, 2);
        default_weights.insert(ScoreType::Gpu, 10);
        default_weights.insert(ScoreType::Disk, 7);
        default_weights.insert(ScoreType::OsBuild, 9);
        default_weights.insert(ScoreType::OtherSystemApi, 7);
        default_weights.insert(ScoreType::Other, 4);
        Self { entries: HashMap::new(), weight: default_weights }
    }
}

impl Score {
    pub fn new(custom_weights: Option<HashMap<ScoreType, u8>>) -> Self {
        match custom_weights {
            Some(weights) => {
                let sanitized_weights = weights.into_iter().map(|(k, v)| (k, v.min(10))).collect();
                Self { entries: HashMap::new(), weight: sanitized_weights }
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

    pub fn calculate_score(&self, typ: Option<ScoreType>) -> f32 {
        let mut fail_prob = 1.0_f32;
        let process_entry = |t: ScoreType, entry: &ScoreEntry, current_prob: &mut f32| {
            let w = self.weight.get(&t).cloned().unwrap_or(5) as f32 / 10.0;
            let base_hit_prob = entry.effective_score() / 10.0;
            let final_hit_prob = base_hit_prob * w;
            *current_prob *= 1.0 - final_hit_prob;
        };
        match typ {
            Some(t) => {
                if let Some(list) = self.entries.get(&t) {
                    for entry in list { process_entry(t, entry, &mut fail_prob); }
                }
            }
            None => {
                for (&t, list) in &self.entries {
                    for entry in list { process_entry(t, entry, &mut fail_prob); }
                }
            }
        }
        let final_score = (1.0 - fail_prob) * 100.0;
        final_score.min(100.0).max(0.0)
    }

    pub fn get_entries(&self) -> &HashMap<ScoreType, Vec<ScoreEntry>> { &self.entries }
    pub fn get_weights(&self) -> &HashMap<ScoreType, u8> { &self.weight }
}


#[derive(Clone, Serialize, Deserialize)]
pub struct EvidenceCollection<T: std::hash::Hash + Eq> {
    pub evidence: HashMap<T, Score>,
    target_weights: HashMap<T, u8>,
    score_weights: Option<HashMap<ScoreType, u8>>,
}
impl<T: std::hash::Hash + Eq> Default for EvidenceCollection<T> {
    fn default() -> Self {
        Self { evidence: HashMap::new(), target_weights: HashMap::new(), score_weights: None }
    }
}

impl<T: std::hash::Hash + Eq + Clone> EvidenceCollection<T> {
    pub fn new(custom_target_weights: Option<HashMap<T, u8>>, custom_score_weights: Option<HashMap<ScoreType, u8>>) -> Self {
        let weights = custom_target_weights.unwrap_or_default()
            .into_iter().map(|(k, v)| (k, v.min(10))).collect();
        Self { evidence: HashMap::new(), target_weights: weights, score_weights: custom_score_weights }
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
        } else { 0.0 }
    }

    pub fn score(&self) -> f32 {
        let mut safe_prob = 1.0_f32;
        for key in self.evidence.keys() {
            let target_score = self.get_score_for(key);
            safe_prob *= 1.0 - (target_score / 100.0);
        }
        ((1.0 - safe_prob) * 100.0).clamp(0.0, 100.0)
    }

    pub fn is_empty(&self) -> bool { self.evidence.is_empty() }
    pub fn get_target_weights(&self) -> &HashMap<T, u8> { &self.target_weights }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, Debug)]
pub enum SandboxType { #[default] Cuckoo, Threatbook, CAPE, Zenbox, JoeSandbox, Unknown }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, Debug)]
pub enum EmulatorType { #[default] Bochs, QemuTCG, Unicorn, Wine, Unknown }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, Debug)]
pub enum VirtualMachineType { #[default] VMware, VirtualBox, HyperV, Xen, KVM, Parallels, Unknown }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, Debug)]
pub enum ContainerType { #[default] Docker, Podman, LXC, Containerd, Kubernetes, Wsl, Unknown }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, Debug)]
pub enum SoftwareType { #[default] Analysis, Debugger, Security }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, Debug)]
pub enum AbnormalType {
    #[default]
    Unknown,
    /// 时间异常
    Time,
    /// 硬件异常：例如 CPU 核心数 <= 1、内存 < 2GB
    Hardware,
    /// 不合预期的系统 API返回
    SystemApi,
    /// 网络异常：例如 DNS 总是解析到同一个 IP（FakeNet 行为）、不存在的网站访问成功
    Network,
    /// 系统信息不一致异常
    Inconsistent,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, Debug)]
pub enum TrustType {
    #[default]
    PersonalFiles,
    UserTraces, UserAccounts, Game, Development,
    InstalledSoftware,
    PhysicalDevices,
    Time,
    Network,
}

pub trait IntoScoreAction {
    fn into_action(self, score_type: ScoreType, msg: Cow<'static, str>, score: u8, confidence: f32) -> ScoreAction;
}
impl IntoScoreAction for SandboxType {
    fn into_action(self, score_type: ScoreType, msg: Cow<'static, str>, score: u8, confidence: f32) -> ScoreAction {
        ScoreAction::Sandbox(self, score_type, msg, score, confidence)
    }
}
impl IntoScoreAction for VirtualMachineType {
    fn into_action(self, score_type: ScoreType, msg: Cow<'static, str>, score: u8, confidence: f32) -> ScoreAction {
        ScoreAction::VirtualMachine(self, score_type, msg, score, confidence)
    }
}
impl IntoScoreAction for EmulatorType {
    fn into_action(self, score_type: ScoreType, msg: Cow<'static, str>, score: u8, confidence: f32) -> ScoreAction {
        ScoreAction::Emulator(self, score_type, msg, score, confidence)
    }
}
impl IntoScoreAction for ContainerType {
    fn into_action(self, score_type: ScoreType, msg: Cow<'static, str>, score: u8, confidence: f32) -> ScoreAction {
        ScoreAction::Container(self, score_type, msg, score, confidence)
    }
}
impl IntoScoreAction for SoftwareType {
    fn into_action(self, score_type: ScoreType, msg: Cow<'static, str>, score: u8, confidence: f32) -> ScoreAction {
        ScoreAction::Software(self, score_type, msg, score, confidence)
    }
}
impl IntoScoreAction for AbnormalType {
    fn into_action(self, score_type: ScoreType, msg: Cow<'static, str>, score: u8, confidence: f32) -> ScoreAction {
        ScoreAction::Abnormal(self, score_type, msg, score, confidence)
    }
}
impl IntoScoreAction for TrustType {
    fn into_action(self, score_type: ScoreType, msg: Cow<'static, str>, score: u8, confidence: f32) -> ScoreAction {
        ScoreAction::Trust(self, score_type, msg, score, confidence)
    }
}


#[derive(Default, Serialize, Deserialize)]
pub struct Environment {
    pub sandbox:        EvidenceCollection<SandboxType>,
    pub virtual_machine:EvidenceCollection<VirtualMachineType>,
    pub emulator:       EvidenceCollection<EmulatorType>,
    pub container:      EvidenceCollection<ContainerType>,
    pub software:       EvidenceCollection<SoftwareType>,
    pub abnormal:       EvidenceCollection<AbnormalType>,
    pub trust:          EvidenceCollection<TrustType>,
    pub risk_weight:  f32,
    pub trust_weight: f32,
}

impl Environment {
    pub fn new() -> Arc<Mutex<Self>> {
        let mut sandbox_weights = HashMap::new();
        sandbox_weights.insert(SandboxType::CAPE, 10);
        sandbox_weights.insert(SandboxType::Threatbook, 10);
        sandbox_weights.insert(SandboxType::Cuckoo, 9);
        sandbox_weights.insert(SandboxType::JoeSandbox, 9);
        sandbox_weights.insert(SandboxType::Zenbox, 9);
        sandbox_weights.insert(SandboxType::Unknown, 6);

        let mut emulator_weights = HashMap::new();
        emulator_weights.insert(EmulatorType::QemuTCG, 8);
        emulator_weights.insert(EmulatorType::Bochs, 8);
        emulator_weights.insert(EmulatorType::Unicorn, 8);
        emulator_weights.insert(EmulatorType::Wine, 8);
        emulator_weights.insert(EmulatorType::Unknown, 6);

        let mut vm_weights = HashMap::new();
        vm_weights.insert(VirtualMachineType::VMware, 8);
        vm_weights.insert(VirtualMachineType::VirtualBox, 8);
        vm_weights.insert(VirtualMachineType::HyperV, 6);
        vm_weights.insert(VirtualMachineType::Unknown, 4);

        let mut container_weights = HashMap::new();
        container_weights.insert(ContainerType::Docker, 5);
        container_weights.insert(ContainerType::Wsl, 5);
        container_weights.insert(ContainerType::Unknown, 1);

        let mut sw_weights = HashMap::new();
        sw_weights.insert(SoftwareType::Debugger, 10);
        sw_weights.insert(SoftwareType::Analysis, 6);
        sw_weights.insert(SoftwareType::Security, 6);

        let mut trust_weights = HashMap::new();
        trust_weights.insert(TrustType::PhysicalDevices, 10);
        trust_weights.insert(TrustType::Game, 10);
        trust_weights.insert(TrustType::Network, 7);
        trust_weights.insert(TrustType::InstalledSoftware, 7);
        trust_weights.insert(TrustType::UserAccounts, 6);
        trust_weights.insert(TrustType::Time, 4);
        trust_weights.insert(TrustType::PersonalFiles, 4);

        let mut abnormal_weights = HashMap::new();
        abnormal_weights.insert(AbnormalType::Inconsistent, 8);
        abnormal_weights.insert(AbnormalType::SystemApi, 9);
        abnormal_weights.insert(AbnormalType::Network, 8);
        abnormal_weights.insert(AbnormalType::Time, 8);
        abnormal_weights.insert(AbnormalType::Hardware, 6);
        abnormal_weights.insert(AbnormalType::Unknown, 4);


        let env = Self {
            sandbox:        EvidenceCollection::new(Some(sandbox_weights), None),
            virtual_machine:EvidenceCollection::new(Some(vm_weights), None),
            emulator:       EvidenceCollection::new(Some(emulator_weights), None),
            container:      EvidenceCollection::new(Some(container_weights), None),
            software:       EvidenceCollection::new(Some(sw_weights), None),
            abnormal:       EvidenceCollection::new(Some(abnormal_weights), None),
            trust:          EvidenceCollection::new(Some(trust_weights), None),
            risk_weight:  1.0,
            trust_weight: 0.6,
        };
        Arc::new(Mutex::new(env))
    }


    pub fn base_risk_score(&self) -> f32 {
        let mut global_safe_prob = 1.0_f32;
        for score in [
            self.sandbox.score(), self.virtual_machine.score(),
            self.emulator.score(), self.container.score(), self.software.score(),
            self.abnormal.score(),
        ] {
            global_safe_prob *= 1.0 - (score / 100.0);
        }
        ((1.0 - global_safe_prob) * 100.0).clamp(0.0, 100.0)
    }

    pub fn trust_score(&self) -> f32 { self.trust.score() }

    pub fn final_risk_score(&self) -> f32 {
        let risk = self.base_risk_score() * self.risk_weight;
        let trust = self.trust_score();
        if risk <= 0.0 { return 0.0; }
        let risk_ratio = risk / 100.0;
        let k = 8.0_f32;
        let x0 = 0.4_f32;
        let mitigation_efficiency = 1.0_f32 / (1.0_f32 + (k * (risk_ratio - x0)).exp());
        let trust_mitigation = trust * self.trust_weight * mitigation_efficiency;
        (risk - risk * (trust_mitigation / 100.0)).clamp(0.0, 100.0)
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
    // <-- 新增：提供 Abnormal 的便捷添加接口
    pub fn abnormal<S: Into<String>>(&mut self, key: AbnormalType, typ: ScoreType, msg: S, score: u8, confidence: f32) {
        self.abnormal.add(key, typ, msg, score, confidence);
    }
    pub fn trust<S: Into<String>>(&mut self, key: TrustType, typ: ScoreType, msg: S, score: u8, confidence: f32) {
        self.trust.add(key, typ, msg, score, confidence);
    }

    pub fn add(&mut self, action: ScoreAction) {
        match action {
            ScoreAction::Sandbox(key, typ, msg, score, confidence)        => self.sandbox(key, typ, msg, score, confidence),
            ScoreAction::Trust(key, typ, msg, score, confidence)          => self.trust(key, typ, msg, score, confidence),
            ScoreAction::VirtualMachine(key, typ, msg, score, confidence) => self.virtual_machine(key, typ, msg, score, confidence),
            ScoreAction::Emulator(key, typ, msg, score, confidence)       => self.emulator(key, typ, msg, score, confidence),
            ScoreAction::Container(key, typ, msg, score, confidence)      => self.container(key, typ, msg, score, confidence),
            ScoreAction::Software(key, typ, msg, score, confidence)       => self.software(key, typ, msg, score, confidence),
            ScoreAction::Abnormal(key, typ, msg, score, confidence)       => self.abnormal(key, typ, msg, score, confidence), // <-- 新增
        }
    }

    pub fn add_all(&mut self, actions: Vec<ScoreAction>) {
        for action in actions { self.add(action); }
    }

    // ──────────────────────────────────────────────────────────
    // Report
    // ──────────────────────────────────────────────────────────

    pub fn dump_report(&self) -> String {
        use std::fmt::Write;
        let mut out = String::with_capacity(4096);

        let base        = self.base_risk_score();
        let trust       = self.trust_score();
        let final_score = self.final_risk_score();

        // ── OVERVIEW ──────────────────────────────────────────
        let _ = writeln!(out, "{}", ss!("── OVERVIEW ───────────────────────────────────────────────────"));
        let _ = writeln!(out, "{}", s_add!("Final Risk  ", Self::bar(final_score, 10), "  ", format_args!("{:5.2}", final_score), "%  [", Self::risk_label(final_score), "]"));
        let _ = writeln!(out, "{}", s_add!("Base Risk   ", Self::bar(base, 10), "  ", format_args!("{:5.2}", base), "%"));
        let _ = writeln!(out, "{}", s_add!("Trust       ", Self::bar(trust, 10), "  ", format_args!("{:5.2}", trust), "%  [mitigates risk]"));
        let _ = writeln!(out, "{}", s_add!("Weights     Risk ", format_args!("{:.2}", self.risk_weight), "  /  Trust ", format_args!("{:.2}", self.trust_weight)));
        let _ = writeln!(out, "{}", ss!("───────────────────────────────────────────────────────────────\n"));

        // ── RISK DIMENSIONS ───────────────────────────────────
        let has_risk = !self.sandbox.is_empty() || !self.virtual_machine.is_empty()
            || !self.emulator.is_empty() || !self.container.is_empty()
            || !self.software.is_empty() || !self.abnormal.is_empty();

        let _ = writeln!(out, "{}", ss!("── RISK DIMENSIONS ────────────────────────────────────────────"));
        if has_risk {
            Self::fmt_collection(&mut out, ss!("Sandbox       "), self.sandbox.score(), &self.sandbox, true);
            Self::fmt_collection(&mut out, ss!("VirtualMachine"), self.virtual_machine.score(), &self.virtual_machine, true);
            Self::fmt_collection(&mut out, ss!("Emulator      "), self.emulator.score(), &self.emulator, true);
            Self::fmt_collection(&mut out, ss!("Container     "), self.container.score(), &self.container, true);
            Self::fmt_collection(&mut out, ss!("Software      "), self.software.score(), &self.software, true);
            Self::fmt_collection(&mut out, ss!("Abnormal      "), self.abnormal.score(), &self.abnormal, true);
        } else {
            let _ = writeln!(out, "{}", ss!("(no risk signals detected)"));
        }
        let _ = writeln!(out, "{}", ss!("───────────────────────────────────────────────────────────────\n"));

        // ── TRUST INDICATORS ──────────────────────────────────
        let _ = writeln!(out, "{}", ss!("── TRUST INDICATORS ───────────────────────────────────────────"));
        if !self.trust.is_empty() {
            Self::fmt_collection(&mut out, ss!("Trust Targets"), self.trust_score(), &self.trust, false);
        } else {
            let _ = writeln!(out, "{}", ss!("(no trust signals detected)"));
        }
        let _ = writeln!(out, "{}", ss!("───────────────────────────────────────────────────────────────\n"));

        // ── THREAT SUMMARY ────────────────────────────────────
        let _ = writeln!(out, "{}", ss!("── THREAT SUMMARY ─────────────────────────────────────────────"));
        if has_risk {
            let _ = writeln!(out, "{}", ss!("[!] Active risk signals — review RISK DIMENSIONS above."));
        } else {
            let _ = writeln!(out, "{}", ss!("[*] No active risk dimensions triggered."));
        }

        if !self.trust.is_empty() {
            let _ = writeln!(out, "\n{}", ss!("Top trust contributors:"));
            let mut tops: Vec<(&TrustType, f32)> = self.trust.evidence.keys()
                .map(|k| (k, self.trust.get_score_for(k)))
                .collect();
            tops.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            for (rank, (target, score)) in tops.iter().take(3).enumerate() {
                let top_msg = self.trust.evidence.get(target)
                    .and_then(|s| {
                        s.get_entries().values().flatten()
                            .max_by(|a, b| a.effective_score().partial_cmp(&b.effective_score()).unwrap())
                            .map(|e| Self::truncate(&e.msg, 34))
                    })
                    .unwrap_or_default();
                let _ = writeln!(out, "{}", s_add!("  ", format_args!("{}", rank + 1), ". ", format_args!("{:<20}", format!("{:?}", target)), "  ", format_args!("{:5.1}", score), "  — ", top_msg));
            }
        }
        let _ = writeln!(out, "{}", ss!("───────────────────────────────────────────────────────────────"));
        out
    }

    // ──────────────────────────────────────────────────────────
    // Report helpers
    // ──────────────────────────────────────────────────────────

    #[inline]
    fn bar(score: f32, width: usize) -> String {
        let filled = ((score / 100.0) * width as f32).round() as usize;
        let empty  = width.saturating_sub(filled);
        format!("[{}{}]", "█".repeat(filled), " ".repeat(empty))
    }

    #[inline]
    fn risk_label(score: f32) -> &'static str {
        match score as u32 {
            0      => "CLEAN",
            1..=24 => "LOW",
            25..=49=> "MEDIUM",
            50..=74=> "HIGH",
            _      => "CRITICAL",
        }
    }

    /// 安全截断字符串（按字符截断，防止中文字符引发 Panic）
    fn truncate(s: &str, max: usize) -> String {
        let char_count = s.chars().count();
        if char_count <= max {
            format!("{:<width$}", s, width = max)
        } else {
            format!("{}…", s.chars().take(max - 1).collect::<String>())
        }
    }

    /// 通用证据集合渲染：同时服务于 Risk 和 Trust 维度
    fn fmt_collection<T>(
        out: &mut String,
        label: &str,
        dim_score: f32,
        collection: &EvidenceCollection<T>,
        is_risk: bool,
    ) where T: std::hash::Hash + Eq + Clone + std::fmt::Debug {
        use std::fmt::Write;
        if collection.is_empty() { return; }

        // Risk 需要在最外层展示维度总分
        if is_risk {
            let _ = writeln!(out, "{} {}  {:5.1}",
                             label, Self::bar(dim_score, 10), dim_score);
        }

        // 目标排序
        let mut targets: Vec<(&T, f32)> = collection.evidence.keys()
            .map(|k| (k, collection.get_score_for(k)))
            .collect();
        targets.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        for (target, score) in targets {
            let name = format!("{:?}", target);
            let count: usize = collection.evidence.get(target)
                .map(|s| s.get_entries().values().map(|v| v.len()).sum())
                .unwrap_or(0);

            let target_bar = if is_risk { Self::bar(score, 8) } else { Self::bar(score, 10) };

            // 目标汇总行
            if is_risk {
                let _ = writeln!(out, "  ↳ {:<14} {}  {:5.1}  ({} hits)", name, target_bar, score, count);
            } else {
                let _ = writeln!(out, "{:<20} {}  {:5.1}  ({} hits)", name, target_bar, score, count);
            }

            // 证据明细行
            if let Some(score_obj) = collection.evidence.get(target) {
                let mut merged: HashMap<String, (f32, usize)> = HashMap::new();
                for (stype, entries) in score_obj.get_entries() {
                    for e in entries {
                        // Risk 前缀加上 ScoreType 方便排查，Trust 保持整洁
                        let key = if is_risk { format!("[{:?}] {}", stype, e.msg) } else { e.msg.clone() };
                        let rec = merged.entry(key).or_insert((e.effective_score(), 0));
                        rec.1 += 1;
                    }
                }

                let mut sorted: Vec<_> = merged.into_iter().collect();
                sorted.sort_by(|a, b| b.1.0.partial_cmp(&a.1.0).unwrap());

                let indent = if is_risk { "      " } else { "    " };

                for (label_str, (eff, dup)) in sorted {
                    let label_d = Self::truncate(&label_str, if is_risk { 88 } else { 80 });
                    let dup_s = if dup > 1 { format!(" ×{}", dup) } else { String::new() };
                    // 仅输出有效分(eff)和重复次数
                    let _ = writeln!(out, "{}{}  {:.2}{}", indent, label_d, eff, dup_s);
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum ScoreAction {
    Sandbox(SandboxType, ScoreType, Cow<'static, str>, u8, f32),
    VirtualMachine(VirtualMachineType, ScoreType, Cow<'static, str>, u8, f32),
    Emulator(EmulatorType, ScoreType, Cow<'static, str>, u8, f32),
    Container(ContainerType, ScoreType, Cow<'static, str>, u8, f32),
    Software(SoftwareType, ScoreType, Cow<'static, str>, u8, f32),
    Abnormal(AbnormalType, ScoreType, Cow<'static, str>, u8, f32),
    Trust(TrustType, ScoreType, Cow<'static, str>, u8, f32),
}
impl ScoreAction {
    pub fn set_msg<M: Into<Cow<'static, str>>>(&mut self, new_msg: M) {
        let target_msg = new_msg.into();
        match self {
            ScoreAction::Sandbox(_, _, msg, _, _) => *msg = target_msg,
            ScoreAction::VirtualMachine(_, _, msg, _, _) => *msg = target_msg,
            ScoreAction::Emulator(_, _, msg, _, _) => *msg = target_msg,
            ScoreAction::Container(_, _, msg, _, _) => *msg = target_msg,
            ScoreAction::Software(_, _, msg, _, _) => *msg = target_msg,
            ScoreAction::Abnormal(_, _, msg, _, _) => *msg = target_msg,
            ScoreAction::Trust(_, _, msg, _, _) => *msg = target_msg,
        }
    }
    pub fn set_score(&mut self, new_score: u8) {
        let safe_score = new_score.min(10);
        match self {
            ScoreAction::Sandbox(_, _, _, score, _) => *score = safe_score,
            ScoreAction::VirtualMachine(_, _, _, score, _) => *score = safe_score,
            ScoreAction::Emulator(_, _, _, score, _) => *score = safe_score,
            ScoreAction::Container(_, _, _, score, _) => *score = safe_score,
            ScoreAction::Software(_, _, _, score, _) => *score = safe_score,
            ScoreAction::Abnormal(_, _, _, score, _) => *score = safe_score,
            ScoreAction::Trust(_, _, _, score, _) => *score = safe_score,
        }
    }
    pub fn set_confidence(&mut self, new_confidence: f32) {
        let safe_confidence = new_confidence.clamp(0.0, 1.0);
        match self {
            ScoreAction::Sandbox(_, _, _, _, confidence) => *confidence = safe_confidence,
            ScoreAction::VirtualMachine(_, _, _, _, confidence) => *confidence = safe_confidence,
            ScoreAction::Emulator(_, _, _, _, confidence) => *confidence = safe_confidence,
            ScoreAction::Container(_, _, _, _, confidence) => *confidence = safe_confidence,
            ScoreAction::Software(_, _, _, _, confidence) => *confidence = safe_confidence,
            ScoreAction::Abnormal(_, _, _, _, confidence) => *confidence = safe_confidence, 
            ScoreAction::Trust(_, _, _, _, confidence) => *confidence = safe_confidence,
        }
    }
}

#[macro_export]
macro_rules! action {
    ($action_type:expr, $score_type:expr, $score:expr, $confidence:expr) => {{
        $action_type.into_action($score_type, "".into(), $score, $confidence)
    }};
    ($action_type:expr, $score_type:expr, $msg:expr, $score:expr, $confidence:expr) => {{
        $action_type.into_action($score_type, $msg.into(), $score, $confidence)
    }};
}