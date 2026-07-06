#![allow(unused_doc_comments)]


use lib::runtime::*;
use lib::shell::shell::Shell;
use lib::utils::sys::info::collect_all;

#[main]
async fn main() -> ! {
    let info = collect_all().await;
    println!("{:#?}", info);




    loop {}
}
