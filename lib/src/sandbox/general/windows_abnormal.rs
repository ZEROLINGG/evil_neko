#![cfg(windows)]
#![allow(non_snake_case, non_camel_case_types)]

use crate::action;
use crate::sandbox::*;
use crate::utils::sys::win::{__current_wallpaper_reg, __current_wallpaper_sysapi};

pub async fn check_win_abnormal(env: Arc<Mutex<Environment>>) {
    let wallpaper_reg = __current_wallpaper_reg();
    let wallpaper_sysapi = __current_wallpaper_sysapi();
    match wallpaper_reg {
        None => { env.lock().await.add(action!(
                    AbnormalType::SystemApi,
                    ScoreType::UserActivity,
                    s_add!("[wallpaper] Unable to retrieve registry wallpaper image path"),
                    6,
                    0.5
                )) }
        Some(_) => {}
    }
    match wallpaper_sysapi {
        None => { env.lock().await.add(action!(
                    AbnormalType::SystemApi,
                    ScoreType::UserActivity,
                    s_add!("[wallpaper] Unable to obtain wallpaper image path via system API"),
                    6,
                    0.5
                )) }
        Some(_) => {}
    }
    if wallpaper_reg != wallpaper_sysapi {
        env.lock().await.add(action!(
                    AbnormalType::Inconsistent,
                    ScoreType::OtherSystemApi,
                    s_add!("[wallpaper] The wallpaper path returned by the registry and the system API is inconsistent."),
                    6,
                    0.5
                ))
    }

}