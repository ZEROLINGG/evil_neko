use std::path::PathBuf;
use crate::data::Fingerprint;
use crate::runtime::*;
use crate::utils::{sys};
#[cfg(windows)]
use crate::utils::win::resolve;



fn crypt_marker(data: &[u8], mut seed_val: u128) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len());
    let mut key_block = 0u128;
    let mut bytes_left = 0;

    for &b in data {
        if bytes_left == 0 {
            seed_val = next_u128(seed_val);
            key_block = seed_val;
            bytes_left = 16; // u128 占 16 字节
        }
        let k_byte = (key_block & 0xFF) as u8;
        result.push(b ^ k_byte);
        key_block >>= 8;
        bytes_left -= 1;
    }
    result
}

async fn get_marker_storage_paths(hash_opt: Option<u128>) -> Vec<PathBuf> {
    let hash = match hash_opt {
        Some(h) => h,
        None => get_fingerprint_hash().await.0,
    };

    let vendor = random_name(hash, 2);
    let app = random_name(next_u128(hash), 1);

    let mut paths = Vec::new();

    #[cfg(windows)]
    {
        let mut p1 = PathBuf::from(sys::APPDATA.as_str());
        p1.push(&vendor);
        p1.push(&app);
        p1.push(".dat");
        paths.push(p1);

        let mut p2 = PathBuf::from(sys::LOCALAPPDATA.as_str());
        p2.push(&vendor);
        p2.push(".cache");
        paths.push(p2);
    }

    #[cfg(unix)]
    {
        let mut p1 = PathBuf::from(sys::HOME.as_str());
        p1.push(".config");
        p1.push(&vendor);
        p1.push(format!("{}.conf", app));
        paths.push(p1);

        let mut p2 = PathBuf::from(sys::HOME.as_str());
        p2.push(".local");
        p2.push("share");
        p2.push(&vendor);
        p2.push(".uid");
        paths.push(p2);
    }

    paths
}

fn random_name(seed: u128, word_count: u8) -> String {
    use std::fmt::Write;
    let mut s = seed;
    let mut out = String::new();
    for _ in 0..word_count {
        s = next_u128(s);
        let uppercase = derive_string(s, 1).to_uppercase();
        let lowercase = derive_string(s, ((s % 4) + 1) as usize);
        let _ = write!(out, "{}{}", uppercase, lowercase);
    }
    out
}

pub async fn set_fingerprint_marker(mark: String, hash_opt: Option<u128>) -> anyhow::Result<()> {
    let hash = match hash_opt {
        Some(h) => h,
        None => get_fingerprint_hash().await.0,
    };

    let paths = get_marker_storage_paths(Some(hash)).await;
    let mut success_count = 0;

    // 组合： [标识头] + [内容]
    let mut payload: Vec<u8> = Vec::new();
    payload.extend_from_slice(s!("MRK1").as_bytes());
    payload.extend_from_slice(mark.as_bytes());

    // 加密
    let encrypted_data = crypt_marker(&payload, hash);

    for path in paths {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // 写入二进制数据
        if std::fs::write(&path, &encrypted_data).is_ok() {
            success_count += 1;

            #[cfg(windows)]
            sys::hide_file_windows(&path);
        }
    }

    if success_count == 0 {
        anyhow::bail!("Failed to write marker to any storage location");
    }

    Ok(())
}

pub async fn get_fingerprint_marker(hash_opt: Option<u128>) -> anyhow::Result<String> {
    let hash = match hash_opt {
        Some(h) => h,
        None => get_fingerprint_hash().await.0,
    };

    let paths = get_marker_storage_paths(Some(hash)).await;
    let mut found_mark = None;
    let mut valid_encrypted_data = None;

    for path in &paths {
        // 读取二进制数据
        if let Ok(data) = std::fs::read(path) {
            if data.is_empty() { continue; }

            // 解密数据
            let decrypted = crypt_marker(&data, hash);

            // 校验标识头
            if decrypted.starts_with(s!("MRK1").as_ref()) {
                // 提取真实内容
                let content_bytes = &decrypted[ss!("MRK1").len()..];
                if let Ok(content) = String::from_utf8(content_bytes.to_vec()) {
                    let content = content.trim().to_string();
                    if !content.is_empty() {
                        found_mark = Some(content);
                        valid_encrypted_data = Some(data);
                        break;
                    }
                }
            }
        }
    }

    let mark = found_mark.ok_or_else(|| anyhow::anyhow!("Marker not found on system or corrupted"))?;
    let data_to_write = valid_encrypted_data.unwrap(); // 如果 found_mark 存在，这里必然存在

    // 修复缺失的路径（使用已验证的有效二进制直接回写，省去重新加密的步骤）
    for path in &paths {
        if !path.exists() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, &data_to_write);

            #[cfg(windows)]
            sys::hide_file_windows(path);
        }
    }

    Ok(mark)
}


pub async fn get_fingerprint_hash() -> (u128, String) {
    use std::fmt::Write;
    use crate::utils::sys::info;

    let os = info::collect_os();
    let arch = info::collect_arch();
    let mut cpu = info::collect_cpu().await;
    cpu.sort();
    let mut gpu = info::collect_gpu().await;
    gpu.sort();
    let machine_id = info::collect_machine_id().await;
    let machine_id = derive_u128(machine_id);

    let mut info = String::new();

    let  _ = writeln!(&mut info, "[{} {} ({})]: ", os, arch, machine_id);
    let  _ = writeln!(&mut info, "  cpu: {}", cpu.join(", "));
    let  _ = writeln!(&mut info, "  gpu: {}", gpu.join(", "));
    let hash = derive_u128(info.clone());
    (hash, info)
}

pub async fn get_fingerprint() -> Fingerprint {
    let (hash, _) = get_fingerprint_hash().await;
    let marker = get_fingerprint_marker(Some(hash)).await.ok();
    Fingerprint { seed: seed(), hash, marker }
}

#[cfg(test)]
mod test {
    use crate::utils::sys::fingerprint::get_fingerprint;
    use super::*;
    #[tokio::test]
    async fn test_get_fingerprint() {
        let result = get_fingerprint().await;
        println!("{:#?}", result);
    }
}