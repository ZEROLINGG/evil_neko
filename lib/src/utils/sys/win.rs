use crate::utils::win::{reg::*, resolve, win_types::registry};
use std::ffi::c_void;
use std::fmt;

// T117230DE0AEA8FB8BFB04ED31D5F18FA6DAF471620161965650FCA79D840F3B1095870B  win11默认壁纸的图片指纹
pub fn __current_wallpaper_reg() -> Option<String> {
    let key = RegKey::open(
        registry::HKEY_CURRENT_USER,
        r"Control Panel\Desktop",
        0,
        registry::KEY_READ,
    )
        .ok()?;
    match key.query_value("Wallpaper").ok()? {
        RegValue::String(s) | RegValue::ExpandString(s) => {
            if s.is_empty() { None } else { Some(s) }
        },
        _ => None,
    }
}
pub fn __current_wallpaper_sysapi() -> Option<String> {
    pub type FnSystemParametersInfoW = unsafe extern "system" fn(u32, u32, *mut c_void, u32) -> i32;
    let spi: FnSystemParametersInfoW = resolve("user32.dll", "SystemParametersInfoW")?;
    const SPI_GETDESKWALLPAPER: u32 = 0x0073;
    let mut buf = [0u16; 260];
    unsafe { spi(SPI_GETDESKWALLPAPER, buf.len() as u32, buf.as_mut_ptr() as _, 0,); }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let path = String::from_utf16_lossy(&buf[..len]);
    if path.is_empty() { None } else { Some(path) }
}
#[derive(Debug, Clone)]
pub struct WindowsVersion {
    pub major: u32,           // 主版本号 (如 10)
    pub minor: u32,           // 次版本号 (如 0)
    pub build: u32,           // Build 号 (如 19045)
    pub ubr: u32,             // Update Build Revision (如 3803)
    pub product_name: String, // 产品名称 (如 "Windows 10 Pro")
    pub display_version: String, // 显示版本 (如 "22H2")
    pub release_id: String,   // 发布 ID (如 "2009")
    pub edition_id: String,   // 版本 ID (如 "Professional")
}
impl WindowsVersion {
    pub fn version(&self) -> String {
        format!("{} {}", self.product_name, self.display_version)
    }
    pub fn build(&self) -> String {
        self.build.to_string()
    }
    pub fn full_build(&self) -> String {
        format!("{}.{}", self.build, self.ubr)
    }

}
impl fmt::Display for WindowsVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.version(), self.full_build())
    }
}


pub fn get_os_version_reg() -> Option<WindowsVersion> {
    let access = registry::KEY_READ | registry::KEY_WOW64_64KEY;
    let key = RegKey::open(
        registry::HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        0,
        access,
    ).ok()?;

    let get_string = |name: &str| -> String {
        key.query_value(name)
            .ok()
            .and_then(|v| v.as_string().map(|s| s.to_string()))
            .unwrap_or_default()
    };

    let get_dword = |name: &str| -> Option<u32> {
        key.query_value(name).ok().and_then(|v| v.as_dword())
    };

    let build_str = get_string("CurrentBuildNumber");
    let build = build_str.parse::<u32>().unwrap_or(0);

    let ubr = get_dword("UBR").unwrap_or(0);

    // Win10+ 存在 CurrentMajorVersionNumber / CurrentMinorVersionNumber
    let mut major = get_dword("CurrentMajorVersionNumber").unwrap_or(0);
    let mut minor = get_dword("CurrentMinorVersionNumber").unwrap_or(0);

    // 如果是老系统 (Win7/8)，上述键不存在，需从 CurrentVersion (如 "6.1") 解析
    if major == 0 && minor == 0 {
        let cv = get_string("CurrentVersion");
        let parts: Vec<&str> = cv.split('.').collect();
        if parts.len() >= 2 {
            major = parts[0].parse().unwrap_or(0);
            minor = parts[1].parse().unwrap_or(0);
        }
    }

    let mut product_name = get_string("ProductName").replace("Microsoft ","");
    if build >= 22000 {
        product_name = product_name
            .replace("Windows 10", "Windows 11");
    }

    let release_id = get_string("ReleaseId");
    let mut display_version = get_string("DisplayVersion");

    if display_version.is_empty() {
        display_version = release_id.clone();
    }

    let edition_id = get_string("EditionID");

    if build == 0 && product_name.is_empty() {
        return None;
    }


    Some(WindowsVersion {
        major,
        minor,
        build,
        ubr,
        product_name,
        display_version,
        release_id,
        edition_id,
    })
}
