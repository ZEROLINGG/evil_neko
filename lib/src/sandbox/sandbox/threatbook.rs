#![cfg(windows)]
#![allow(non_snake_case, non_camel_case_types)]

use crate::action;
use crate::sandbox::utils::fs::{__dir_diff, __image_diff};
use crate::sandbox::*;
use crate::utils::sys::win::get_os_version_reg;

pub async fn check_threatbook(env: Arc<Mutex<Environment>>) {
    let images: [HeapStr;4] = [
        s!(r"C:\Users\Administrator\AppData\Local\Microsoft\Windows\Themes\RoamedThemeFiles\DesktopBackground\Desktop.jpg").into(),
        s!(r"C:\Users\Administrator\AppData\Local\Microsoft\Windows\Themes\RoamedThemeFiles\DesktopBackground\img0.jpg").into(),
        s!(r"C:\Windows\Web\Wallpaper\Windows\img0.jpg").into(),
        s!(r"C:\Windows\Web\Wallpaper\Windows\Desktop.jpg").into(),

    ];
    for img in images.iter() {
        if let Some(a) = __image_diff(
            img.clone(),
            s!("T11523BC7026C4C72D0A4DBB40CD31F53203B2428351B19E45B9FA1B88A60BBE027006EF"),
            20,
            action!(
                SandboxType::Threatbook,
                ScoreType::StrongFingerprint,
                10,
                1.0
            ),
        )
        .await
        {
            env.lock().await.add(a);
        }
    }

    if let Some(sys) = get_os_version_reg() {
        if sys.major == 10
            && sys.build == 18362
            && sys.edition_id == "Professional"
            && sys.release_id == "1903"
            && sys.ubr == 30
            && sys.minor == 0
        {
            env.lock().await.add(action!(
                SandboxType::Threatbook,
                ScoreType::OsBuild,
                s_add!("threatbook os build match", sys),
                9,
                0.8
            ));
        }
    }

    match __dir_diff(
        s!(r"C:\Users\Administrator\Desktop"),
        s!(r"T135C012E1A08478A296E09296DA395D6E374A4D8640D8F611516D8A6408923312A92BA5"),
        5,
        action!(SandboxType::Threatbook, ScoreType::Directory, 6, 0.3),
    )
    .await
    {
        None => {}
        Some(a) => {
            env.lock().await.add(a);
        }
    }

    match __dir_diff(
        s!(r"C:\Users\Administrator\AppData\Roaming\Microsoft\Windows\Recent"),
        s!(r"T14A91A80380FDB89188E40728613D7F4EAF309DABB8A5D69B006DC3C314864A699F7057"),
        20,
        action!(SandboxType::Threatbook, ScoreType::Directory, 6, 0.3),
    )
        .await
    {
        None => {}
        Some(a) => {
            env.lock().await.add(a);
        }
    }
    // TODO
}
