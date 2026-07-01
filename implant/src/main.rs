#![allow(unused_doc_comments)]

use lib::pm::sprint;
use lib::sandbox::emulator::wine::check_wine;
use lib::sandbox::Environment;
use lib::sandbox::general::windows_abnormal::check_win_abnormal;
use lib::sandbox::sandbox::threatbook::check_threatbook;
use lib::sandbox::utils::fs::___tlsh_dir_string;
use lib::utils::sys::win::get_os_version_reg;

#[lib::pm::rt]
#[lib::main]
async fn main() -> ! {
    let env = Environment::new();
    #[cfg(windows)]
    {
        lib::sandbox::general::windows_media_foundation::check_wmf_hardware(env.clone()).await;
        lib::sandbox::general::windows_debugger::check_win_debugger(env.clone()).await;
        check_wine(env.clone()).await;
        check_win_abnormal(env.clone()).await;
        check_threatbook(env.clone()).await;
    }

    lib::sandbox::general::general_debgger::check_debugger(env.clone()).await;
    lib::sandbox::general::hostname::check_hostname(env.clone()).await;
    lib::sandbox::general::username::check_username(env.clone()).await;
    sprint!(env.lock().await.dump_report());
    println!("C:\\Users\\Administrator\\AppData\\Roaming\\Microsoft\\Windows\\Recent : {:?}",___tlsh_dir_string(r"C:\Users\Administrator\AppData\Roaming\Microsoft\Windows\Recent").await);

    loop {}
}
