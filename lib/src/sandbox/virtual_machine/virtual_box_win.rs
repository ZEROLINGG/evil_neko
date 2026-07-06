#![cfg(windows)]
use crate::sandbox::*;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn check_vbox_win(_env: Arc<Mutex<Environment>>) {
    // c盘根目录可能存在vboxpostinstall.log

    // 存在vboxservice.exe，vboxtray.exe

// PS D:\> ls
    //
    //
    //     目录: D:\
    //
    //
    // Mode                 LastWriteTime         Length Name
    // ----                 -------------         ------ ----
    // d-----         2026/4/18     18:51                NT3x
    // d-----         2026/4/18     18:51                OS2
    // d-----         2026/4/18     18:51                cert
    // --r---        2025/11/15      0:56           1049 AUTORUN.INF
    // --r---         2026/4/18     18:50        2209805 VBoxDarwinAdditions.pkg
    // --r---         2026/4/18     18:50           4225 VBoxDarwinAdditionsUninstall.tool
    // --r---         2026/4/18     18:50        6737577 VBoxLinuxAdditions.run
    // --r---         2026/4/18     18:50        2959038 VBoxLinuxAdditions-arm64.run
    // --r---         2026/4/18     18:50        9583616 VBoxSolarisAdditions.pkg
    // --r---         2026/4/18     18:15        1079568 VBoxWindowsAdditions.exe
    // --r---         2026/4/18     18:50        8725776 VBoxWindowsAdditions-amd64.exe
    // --r---         2026/4/18     18:42        5060712 VBoxWindowsAdditions-arm64.exe
    // --r---         2026/4/18     18:47        6770864 VBoxWindowsAdditions-x86.exe
    // --r---         2026/4/18     18:50           7114 autorun.sh
    // --r---         2026/4/18     18:50           5097 runasroot.sh
    // --r---        2025/11/15      0:56            261 windows11-bypass.reg
    //
    //
    // PS D:\> ls NT3x
    //
    //
    //     目录: D:\NT3x
    //
    //
    // Mode                 LastWriteTime         Length Name
    // ----                 -------------         ------ ----
    // --r---         2026/1/14      2:39           2099 Readme.txt
    // --r---         2026/4/18     18:14         243192 VBoxAddInstallNt3x.exe
    // --r---         2026/4/18     18:47         617784 VBoxControl.exe
    // --r---         2026/4/18     18:47         257352 VBoxGuest.sys
    // --r---         2026/4/18     18:14         208408 VBoxMouseNT.sys
    // --r---         2026/4/18     18:14         881168 VBoxService.exe
    //
    //
    // PS D:\> ls OS2
    //
    //
    //     目录: D:\OS2
    //
    //
    // Mode                 LastWriteTime         Length Name
    // ----                 -------------         ------ ----
    // --r---         2026/4/18     18:50        2108368 VBoxControl.exe
    // --r---         2026/4/18     18:50         579654 VBoxGuest.sys
    // --r---         2026/4/18     18:50          58713 VBoxMouse.sys
    // --r---         2026/4/18     18:50          18130 VBoxOs2AdditionsInstall.exe
    // --r---         2026/4/18     18:50           6080 VBoxReplaceDll.exe
    // --r---         2026/4/18     18:50         704167 VBoxSF.ifs
    // --r---         2026/4/18     18:50        1369167 VBoxService.exe
    // --r---         2026/4/18     18:50          66343 gengradd.dll
    // --r---         2026/4/18     18:50          48179 libc06.dll
    // --r---         2026/4/18     18:50          48179 libc061.dll
    // --r---         2026/4/18     18:50         157161 libc062.dll
    // --r---         2026/4/18     18:50         157161 libc063.dll
    // --r---         2026/4/18     18:50         157213 libc064.dll
    // --r---         2026/4/18     18:50         157213 libc065.dll
    // --r---         2026/4/18     18:50        1361666 libc066.dll
    // --r---         2026/4/18     18:50           1786 readme.txt
    //
    //
    // PS D:\> ls cert
    //
    //
    //     目录: D:\cert
    //
    //
    // Mode                 LastWriteTime         Length Name
    // ----                 -------------         ------ ----
    // --r---         2026/4/18     18:14         655656 VBoxCertUtil.exe
    // --r---         2026/4/18     18:16           1419 vbox-sha1.cer
    // --r---         2026/4/18     18:16           1239 vbox-sha1-root.cer
    // --r---         2026/4/18     18:16           1603 vbox-sha1-timestamp-root.cer
    // --r---         2026/4/18     18:16           1779 vbox-sha256.cer
    // --r---         2026/4/18     18:16           1428 vbox-sha256-root.cer
    // --r---         2026/4/18     18:16           1428 vbox-sha256-timestamp-root.cer

    // PS D:\> dir C:\Windows\System32 | Where-Object {$_.Name -like "*vbox*"}
    //
    //
    //     目录: C:\Windows\System32
    //
    //
    // Mode                 LastWriteTime         Length Name
    // ----                 -------------         ------ ----
    // -a----         2026/4/18      9:27         698336 VBoxControl.exe
    // -a----         2026/4/18      9:27         445848 VBoxDispD3D.dll
    // -a----         2026/4/18      9:27         493216 VBoxDX.dll
    // -a----         2026/4/18      9:27       11902440 VBoxGL.dll
    // -a----         2026/4/18     18:16          36536 VBoxHook.dll
    // -a----         2026/4/18      9:27         393928 VBoxMRXNP.dll
    // -a----         2026/4/18      9:27        6756768 VBoxNine.dll
    // -a----         2026/4/18     18:16         972600 VBoxService.exe
    // -a----         2026/4/18      9:27        6075640 VBoxSVGA.dll
    // -a----         2026/4/18      9:27         939328 VBoxTray.exe


}
