use std::borrow::Cow;
use anyhow::Result;

#[derive(Clone, Debug)]
pub struct Line<'a> {
    pub raw: Cow<'a, str>,              // 原始行内容 ($0)
    pub fields: Vec<Cow<'a, str>>,      // 字段列表 ($1, $2, ...)
}

#[derive(Clone, Debug)]
pub struct AwkResult<'a> {
    pub lines: Vec<Line<'a>>,
}

impl<'a> Line<'a> {
    pub fn get(&self, index: usize) -> Option<&str> {
        if index == 0 {
            Some(&self.raw)
        } else {
            self.fields.get(index - 1).map(|s| s.as_ref())
        }
    }
    pub fn nf(&self) -> usize { self.fields.len() }
}

impl<'a> AwkResult<'a> {
    pub fn nr(&self) -> usize { self.lines.len() }
    pub fn line(&self, index: usize) -> Option<&Line<'a>> {
        if index == 0 {
            None
        } else {
            self.lines.get(index - 1)
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = (usize, &Line<'a>)> {
        self.lines.iter().enumerate().map(|(i, line)| (i + 1, line))
    }

    pub fn filter(&self, filter: impl Fn(&Line<'a>) -> bool) -> Vec<Line<'a>> {
        self.lines
            .iter()
            .filter(|&line| filter(line))
            .cloned()
            .collect()
    }
}

pub fn awk<'a>(
    input: impl Into<Cow<'a, str>>,
    field_sep: impl IntoIterator<Item = impl AsRef<str>>, // 空则默认空格加制表符
    line_sep: impl IntoIterator<Item = impl AsRef<str>>,  // 空则默认\n
) -> Result<AwkResult<'a>> {
    let input_cow = input.into();

    let mut l_seps: Vec<String> = line_sep.into_iter().map(|s| s.as_ref().to_string()).collect();
    if l_seps.is_empty() {
        l_seps.push("\n".to_string());
    }

    let f_seps: Vec<String> = field_sep.into_iter().map(|s| s.as_ref().to_string()).collect();
    let use_default_fs = f_seps.is_empty();

    let raw_lines = split_cow_by_seps(&input_cow, &l_seps);

    let mut lines = Vec::with_capacity(raw_lines.len());
    for raw_line in raw_lines {
        let fields = if use_default_fs {
            split_cow_whitespace(&raw_line)
        } else {
            split_cow_by_seps(&raw_line, &f_seps)
        };

        lines.push(Line { raw: raw_line, fields });
    }

    Ok(AwkResult { lines })
}

// ---------------- 辅助函数 ----------------

/// 查找字符串中最早出现的任意一个分隔符
/// 返回 (匹配位置的索引, 分隔符的字节长度)
fn find_first_sep(s: &str, seps: &[String]) -> Option<(usize, usize)> {
    seps.iter()
        .filter_map(|sep| s.find(sep).map(|idx| (idx, sep.len())))
        .min_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
}

/// 根据特定的分隔符集合对 Cow 字符串进行切分
fn split_cow_by_seps<'a>(cow: &Cow<'a, str>, seps: &[String]) -> Vec<Cow<'a, str>> {
    let mut result = Vec::new();

    match cow {
        Cow::Borrowed(original_str) => {
            let mut current = *original_str;
            while let Some((idx, len)) = find_first_sep(current, seps) {
                result.push(Cow::Borrowed(&current[..idx]));
                current = &current[idx + len..];
            }
            result.push(Cow::Borrowed(current));
        }
        Cow::Owned(original_string) => {
            let mut current = original_string.as_str();
            while let Some((idx, len)) = find_first_sep(current, seps) {
                result.push(Cow::Owned(current[..idx].to_string()));
                current = &current[idx + len..];
            }
            result.push(Cow::Owned(current.to_string()));
        }
    }
    result
}

/// 默认 AWK 行为：按连续的空白字符（空格/制表符等）切分，忽略前后空白
fn split_cow_whitespace<'a>(cow: &Cow<'a, str>) -> Vec<Cow<'a, str>> {
    match cow {
        Cow::Borrowed(s) => s.split_whitespace().map(Cow::Borrowed).collect(),
        Cow::Owned(s) => s
            .split_whitespace()
            .map(|part| Cow::Owned(part.to_string()))
            .collect(),
    }
}