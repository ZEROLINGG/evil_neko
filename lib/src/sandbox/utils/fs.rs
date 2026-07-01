#![allow(unused)]
use crate::sandbox::*;

use std::env as std_env;
use std::str::FromStr;
use std::sync::LazyLock;
use tokio::fs as tokio_fs;
use tokio::io::AsyncReadExt;

static HOME: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("HOME"))
        .or_else(|_| std_env::var(ss!("USERPROFILE")))
        .unwrap_or_else(|_| "/".to_string()).into()
});

static APPDATA: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("APPDATA"))
        .unwrap_or_else(|_| s_add!(HOME.as_str(), r"\AppData\Roaming").into_string())
});

static LOCALAPPDATA: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("LOCALAPPDATA"))
        .unwrap_or_else(|_| s_add!(HOME.as_str(), r"\AppData\Local").into_string())
});

static DESKTOP: LazyLock<String> = LazyLock::new(|| {
    s_add!(HOME.as_str(), r"\Desktop").into_string()
});

static SYS_DRIVE: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("SystemDrive")).unwrap_or_else(|_| "C:".to_string())
});

static WINDIR: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("windir")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\Windows").into_string())
});

static PROGRAMDATA: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("ProgramData")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\ProgramData").into_string())
});

static PROG_FILES: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("ProgramFiles")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\Program Files").into_string())
});

static PROG_FILES_X86: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("ProgramFiles(x86)")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\Program Files (x86)").into_string())
});

pub(crate) static SYSTEM32: LazyLock<String> = LazyLock::new(|| {
    s_add!(WINDIR.as_str(), r"\System32").into_string()
});

static SYSWOW64: LazyLock<String> = LazyLock::new(|| {
    s_add!(WINDIR.as_str(), r"\SysWOW64").into_string()
});

pub(crate) static TEMP: LazyLock<String> = LazyLock::new(|| {
    std_env::var(ss!("TEMP"))
        .or_else(|_| std_env::var(ss!("TMP")))
        .unwrap_or_else(|_| {
            #[cfg(windows)]
            { s_add!(LOCALAPPDATA.as_str(), r"\Temp").to_string() }
            #[cfg(unix)]
            { sss!("/tmp") }
        })
});

static USERS: LazyLock<String> = LazyLock::new(|| {
    if cfg!(windows) {
        std_env::var(ss!("PUBLIC")).unwrap_or_else(|_| s_add!(SYS_DRIVE.as_str(), r"\Users\Public").into_string())
    } else if cfg!(target_os = "linux") {
        sss!(r"/home")
    } else { sss!("/Users") }
});

pub(crate) async fn __has_file(path: impl Str, min_size: Option<u64>, mut action: ScoreAction) -> Option<ScoreAction> {
    if let Ok(meta) = tokio_fs::symlink_metadata(path.as_str()).await {
        if meta.is_file() {
            let file_size = meta.len();
            if file_size >= min_size.unwrap_or(0) {
                action.set_msg(s_fmt!("File found[{: >6.1}KB]: {}", file_size as f64 / 1024.0, path));
                return Some(action);
            }
        }
    }
    None
}

async fn __has_dir(path: impl Str, mut action: ScoreAction) -> Option<ScoreAction> {
    if let Ok(meta) = tokio_fs::metadata(path.as_str()).await {
        if meta.is_dir() {
            action.set_msg(s_add!("dir found: ", path));
            return Some(action);
        }
    }
    None
}

async fn __dir_subitem_few(
    dir: impl Str,
    mut thresholds: Vec<(u32, ScoreAction)>,
) -> Option<ScoreAction> {
    if thresholds.is_empty() { return None; }
    thresholds.sort_by_key(|(min, _)| *min);

    let max_min = thresholds.last().unwrap().0;

    if let Ok(mut entries) = tokio_fs::read_dir(dir.as_str()).await {
        let mut count: u32 = 0;

        while let Ok(Some(_)) = entries.next_entry().await {
            count += 1;
            if count >= max_min { break; }
        }
        for (min, mut action) in thresholds {
            if count < min {
                action.set_msg(s_add!("Too few files[", count, "]: ", dir));
                return Some(action);
            }
        }
    }
    None
}

async fn __dir_subitem_rich(
    dir: impl Str,
    mut thresholds: Vec<(u32, ScoreAction)>,
) -> Option<ScoreAction> {
    if thresholds.is_empty() { return None; }

    thresholds.sort_by(|a, b| b.0.cmp(&a.0));

    let max_threshold = thresholds.first().unwrap().0;

    if let Ok(mut entries) = tokio_fs::read_dir(dir.as_str()).await {
        let mut count: u32 = 0;

        while let Ok(Some(_)) = entries.next_entry().await {
            count += 1;
            if count >= max_threshold { break; }
        }
        for (min, mut action) in thresholds {
            if count >= min {
                action.set_msg(s_add!("Enough files[", count, "+]: ", dir));
                return Some(action);
            }
        }
    }
    None
}

pub(crate) async fn __file_diff(
    path: impl Str,
    tlsh: impl Str,
    distance: u8,
    mut action: ScoreAction,
) -> Option<ScoreAction> {

    let target = tlsh2::TlshDefault::from_str(tlsh.as_str()).ok()?;

    let hash = ___tlsh_file(path.as_str()).await?;
    let diff = hash.diff(&target, true);

    if diff <= distance.into() {
        action.set_msg(s_fmt!(
            "TLSH matched (distance={}): {}",
            diff,
            path
        ));
        Some(action)
    } else {
        None
    }
}


pub async fn __image_diff(
    path: impl Str,
    tlsh: impl Str,
    distance: u8,
    mut action: ScoreAction,
) -> Option<ScoreAction> {

    let target = tlsh2::TlshDefault::from_str(tlsh.as_str()).ok()?;

    let img = tokio::task::spawn_blocking({
        let path = path.as_str().to_string();
        move || image::open(path).ok()
    })
        .await
        .ok()??;

    let hash = ___tlsh_image(img).await?;
    let diff = hash.diff(&target, true);

    if diff <= distance.into() {
        action.set_msg(s_fmt!(
            "Image TLSH matched (distance={}): {}",
            diff,
            path
        ));
        Some(action)
    } else {
        None
    }
}

async fn ___tlsh_file(file: &str) -> Option<tlsh2::Tlsh128_1> {
    let mut file = tokio_fs::File::open(file).await.ok()?;
    let mut builder = tlsh2::TlshDefaultBuilder::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).await.ok()?;
        if n == 0 {
            break;
        }

        builder.update(&buf[..n]);
    }
    let hash = builder.build()?;
    Some(hash)
}
pub async fn ___tlsh_file_string(file: &str) -> Option<String> {
    Option::from(String::from_utf8_lossy(&___tlsh_file(file).await?.hash()).into_owned())
}
pub async fn ___tlsh_image(img: image::DynamicImage) -> Option<tlsh2::Tlsh128_1> {
    use image::imageops::FilterType;

    tokio::task::spawn_blocking(move || {
        const WIDTH: usize = 128;
        const HEIGHT: usize = 128;
        const PIXELS: usize = WIDTH * HEIGHT;

        let resized = img.resize_exact(
            WIDTH as u32,
            HEIGHT as u32,
            FilterType::Nearest,
        );

        let rgb = resized.into_rgb8();
        let raw_pixels = rgb.as_raw();

        let mut processed = vec![0u8; PIXELS * 3];

        for (i, chunk) in raw_pixels.chunks_exact(3).enumerate() {
            processed[i] = chunk[0] >> 3;
            processed[PIXELS + i] = chunk[1] >> 3;
            processed[PIXELS * 2 + i] = chunk[2] >> 3;
        }

        let mut builder = tlsh2::TlshDefaultBuilder::new();
        builder.update(&processed);
        builder.build()
    })
        .await
        .ok()?
}

/// 收集目录下的子项名称，排序后转为 Vec<u8>，然后用 TLSH 比较
pub async fn __dir_diff(
    dir: impl Str,
    tlsh: impl Str,
    distance: u8,
    mut action: ScoreAction,
) -> Option<ScoreAction> {

    let target = tlsh2::TlshDefault::from_str(tlsh.as_str()).ok()?;

    let hash = ___tlsh_dir(dir.as_str()).await?;
    let diff = hash.diff(&target, true);

    if diff <= distance.into() {
        action.set_msg(s_fmt!(
            "Directory structure TLSH matched (distance={}): {}",
            diff,
            dir
        ));
        Some(action)
    } else {
        None
    }
}

/// 收集目录下的子项名称并生成 TLSH 哈希
async fn ___tlsh_dir(dir: &str) -> Option<tlsh2::Tlsh128_1> {
    let mut entries = tokio_fs::read_dir(dir).await.ok()?;
    let mut names = Vec::new();

    let mut count: u32 = 0;
    while let Ok(Some(entry)) = entries.next_entry().await && count < 10000 {
        if let Ok(name) = entry.file_name().into_string() {
            names.push(name);
            count += 1;
        }
    }

    if names.is_empty() {
        return None;
    }

    names.sort();

    let data = names.join("\n").into_bytes();

    let mut builder = tlsh2::TlshDefaultBuilder::new();
    builder.update(&data);
    builder.build()
}

pub async fn ___tlsh_dir_string(dir: &str) -> Option<String> {
    Option::from(String::from_utf8_lossy(&___tlsh_dir(dir).await?.hash()).into_owned())
}
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs as tokio_fs;
    use tokio::io::AsyncWriteExt;
    use std::time::{SystemTime, UNIX_EPOCH};
    use crate::action;

    fn mock_action() -> ScoreAction {
        action!(SandboxType::Unknown, ScoreType::File, 0, 0.0)
    }

    fn generate_entropy_data(size: usize) -> Vec<u8> {
        let mut data = Vec::with_capacity(size);
        for i in 0..size {
            data.push((i % 256) as u8);
        }
        data
    }

    #[tokio::test]
    async fn test_file_and_dir_existence() {
        let temp_dir = std::path::Path::new(&*TEMP);

        let file_path = temp_dir.join("test_file.txt");
        let file_path_str = file_path.to_str().unwrap().to_string();

        
        let mut file = tokio_fs::File::create(&file_path).await.unwrap();
        file.write_all(&vec![0u8; 2048]).await.unwrap();

        let action = mock_action();

        
        let res_dir = __has_dir(HeapStr::new(temp_dir.to_string_lossy().as_ref()), action.clone()).await;
        assert!(res_dir.is_some(), "临时目录应该被识别到");

        
        let res_file1 = __has_file(HeapStr::new(file_path_str.as_str()), None, action.clone()).await;
        assert!(res_file1.is_some(), "文件应该存在");

        
        let res_file2 = __has_file(HeapStr::new(file_path_str.as_str()), Some(1024), action.clone()).await;
        assert!(res_file2.is_some(), "文件大小 2048 >= 1024，应该匹配成功");

        
        let res_file3 = __has_file(HeapStr::new(file_path_str), Some(4096), action.clone()).await;
        assert!(res_file3.is_none(), "文件大小 2048 < 4096，不应匹配");
    }

    #[tokio::test]
    async fn test_dir_subitem_count() {
        let temp_dir = std::path::Path::new(&*TEMP).join("test_dir_subitem_count").join(format!("{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        let dir_path = temp_dir.to_str().unwrap().to_string();
        tokio_fs::create_dir_all(&dir_path).await.unwrap();
        
        
        for i in 0..3 {
            let fp = temp_dir.join(format!("file_{}.txt", i));
            tokio_fs::File::create(fp).await.unwrap();
        }

        let action = mock_action();

        
        
        let res_few_match = __dir_subitem_few(HeapStr::new(dir_path.as_str()), vec![(5, action.clone())]).await;
        assert!(res_few_match.is_some(), "文件数较少，应该匹配成功");

        
        let res_few_unmatch = __dir_subitem_few(HeapStr::new(dir_path.as_str()), vec![(2, action.clone())]).await;
        assert!(res_few_unmatch.is_none(), "文件数超标，不应触发 few");

        
        
        let res_rich_match = __dir_subitem_rich(HeapStr::new(dir_path.as_str()), vec![(2, action.clone())]).await;
        assert!(res_rich_match.is_some(), "文件数足够多，应该匹配成功");


        let res_rich_unmatch = __dir_subitem_rich(HeapStr::new(dir_path), vec![(5, action.clone())]).await;
        assert!(res_rich_unmatch.is_none(), "文件数不够，不应触发 rich");
    }

    #[tokio::test]
    async fn test_tlsh_file_fingerprint() {
        let temp_dir = std::path::Path::new(&*TEMP);
        let file_path = temp_dir.join("tlsh_target.bin");
        let file_path_str = file_path.to_str().unwrap().to_string();

        
        let data = generate_entropy_data(4096);
        let mut file = tokio_fs::File::create(&file_path).await.unwrap();
        file.write_all(&data).await.unwrap();
        file.sync_all().await.unwrap(); 

        
        let hash_opt = ___tlsh_file(&file_path_str).await;
        assert!(hash_opt.is_some(), "TLSH 哈希生成失败，可能是文件太小或熵值过低");

        let hash = hash_opt.unwrap();
        let hash_str = String::from_utf8_lossy(&hash.hash()).into_owned();

        
        let action = mock_action();
        let diff_match = __file_diff(HeapStr::new(file_path_str.as_str()), HeapStr::new(hash_str.as_str()), 10, action.clone()).await;
        assert!(diff_match.is_some(), "使用同样的 TLSH 字符串进行比较，应该匹配成功");

        
        let fake_tlsh = "T12E41C011EB14EA9B091F89292F78594C17FB40183A9372A7F14E169C4B05B0D276FFE8";
        let diff_unmatch = __file_diff(HeapStr::new(file_path_str.as_str()), HeapStr::new(fake_tlsh), 10, action.clone()).await;
        assert!(diff_unmatch.is_none(), "使用截然不同的 TLSH 进行比较，距离应该大于 10，返回 None");
    }

    #[tokio::test]
    async fn test_tlsh_image_fingerprint() {
        let temp_dir = std::path::Path::new(&*TEMP);
        let img_path = temp_dir.join("test_image.png");
        let img_path_str = img_path.to_str().unwrap().to_string();


        let mut img = image::ImageBuffer::new(200, 200);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            let r = (x % 255) as u8;
            let g = (y % 255) as u8;
            let b = ((x + y) % 255) as u8;
            *pixel = image::Rgb([r, g, b]);
        }
        img.save(&img_path).unwrap();

        let value = img_path_str.clone();
        let img = tokio::task::spawn_blocking({
            move || image::open(value.clone()).ok()
        })
            .await
            .ok().unwrap().unwrap();

        let hash_opt = ___tlsh_image(img).await;
        assert!(hash_opt.is_some(), "图片 TLSH 生成失败");

        let hash = hash_opt.unwrap();
        let hash_str = String::from_utf8_lossy(&hash.hash()).into_owned();


        let action = mock_action();
        let diff_match = __image_diff(HeapStr::new(img_path_str.as_str()), HeapStr::new(hash_str), 5, action.clone()).await;
        assert!(diff_match.is_some(), "相同的图片 TLSH 比较应匹配成功");


        let fake_tlsh = "T18C23DD52AE5FBACFEB506831C1A446528AE9731C46B316A23C6C5A7CD48F3535F3AF10";
        let diff_unmatch = __image_diff(HeapStr::new(img_path_str.as_str()), HeapStr::new(fake_tlsh), 5, action.clone()).await;
        assert!(diff_unmatch.is_none(), "完全无关的图片 TLSH 不应匹配");
    }
}