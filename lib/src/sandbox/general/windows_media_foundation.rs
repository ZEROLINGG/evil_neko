#![cfg(windows)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

use crate::action;
use crate::sandbox::*;
use crate::utils::win::{resolve, win_fn, GUID, HRESULT};
use anyhow::Result;
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;

// ============= COM 初始化标志 =============
pub const COINIT_MULTITHREADED: u32 = 0x0;

// ============= 版本常量 =============
const MF_API_VERSION: u32 = 0x0070;
const MF_SDK_VERSION: u32 = 0x0002;
pub const MF_VERSION: u32 = (MF_SDK_VERSION << 16) | MF_API_VERSION;
pub const MFSTARTUP_FULL: u32 = 0;

// ============= GUID 常量 =============

// Media Types
const MFMediaType_Video: GUID = GUID::from_u128(0x73646976_0000_0010_8000_00aa00389b71);
const MFMediaType_Audio: GUID = GUID::from_u128(0x73647561_0000_0010_8000_00aa00389b71);

// Video Formats
const MFVideoFormat_H264: GUID = GUID::from_u128(0x34363248_0000_0010_8000_00aa00389b71);
const MFVideoFormat_RGB32: GUID = GUID::from_u128(0x00000016_0000_0010_8000_00aa00389b71);
const MFVideoFormat_NV12: GUID = GUID::from_u128(0x3231564e_0000_0010_8000_00aa00389b71);

// MFT Categories
const MFT_CATEGORY_VIDEO_ENCODER: GUID = GUID::from_u128(0xf79eac7d_e545_4387_bdee_d647d7bde42a);

// Media Type Attributes
const MF_MT_MAJOR_TYPE: GUID = GUID::from_u128(0x48eba18e_f8c9_4687_bf11_0a74c9f96a8f);
const MF_MT_SUBTYPE: GUID = GUID::from_u128(0xf7e34c9a_42e8_4714_b74b_cb29d72c35e5);
const MF_MT_AVG_BITRATE: GUID = GUID::from_u128(0x20332624_fb0d_4d9e_bd0d_cbf6786c102e);
const MF_MT_FRAME_SIZE: GUID = GUID::from_u128(0x1652c33d_d6b2_4012_b834_72030849a37d);
const MF_MT_FRAME_RATE: GUID = GUID::from_u128(0xc459a2e8_3d2c_4e44_b132_fee5156c7bb0);
const MF_MT_INTERLACE_MODE: GUID = GUID::from_u128(0xe2724bb8_e676_4806_b4b2_a8d6efb44ccd);
const MF_MT_PIXEL_ASPECT_RATIO: GUID = GUID::from_u128(0xc6376a1e_8d0a_4027_be45_6d9a0ad39bb6);

// Sink Writer Attributes
const MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS: GUID = GUID::from_u128(0xa634a91c_822b_41b9_a494_4de4643612b0);
const MF_SINK_WRITER_DISABLE_THROTTLING: GUID = GUID::from_u128(0x08b845d8_2b74_4afe_9d53_be16d2d5ae4f);

// ============= 枚举和标志 =============

const MFT_ENUM_FLAG_SYNCMFT: u32 = 0x00000001;
const MFT_ENUM_FLAG_ASYNCMFT: u32 = 0x00000002;
const MFT_ENUM_FLAG_HARDWARE: u32 = 0x00000004;
const MFT_ENUM_FLAG_FIELDOFUSE: u32 = 0x00000008;
const MFT_ENUM_FLAG_LOCALMFT: u32 = 0x00000010;
const MFT_ENUM_FLAG_TRANSCODE_ONLY: u32 = 0x00000020;
const MFT_ENUM_FLAG_SORTANDFILTER: u32 = 0x00000040;
const MFT_ENUM_FLAG_ALL: u32 = 0x0000003F;

// Video Interlace Mode
#[repr(C)]
#[derive(Clone, Copy)]
struct MFVideoInterlaceMode(i32);

impl MFVideoInterlaceMode {
    const Unknown: Self = Self(0);
    const Progressive: Self = Self(2);
    const FieldInterleavedUpperFirst: Self = Self(3);
    const FieldInterleavedLowerFirst: Self = Self(4);
    const FieldSingleUpper: Self = Self(5);
    const FieldSingleLower: Self = Self(6);
    const MixedInterlaceOrProgressive: Self = Self(7);
}

const MFVideoInterlace_Progressive: MFVideoInterlaceMode = MFVideoInterlaceMode::Progressive;

// ============= 结构体定义 =============

#[repr(C)]
pub struct MFT_REGISTER_TYPE_INFO {
    pub guidMajorType: GUID,
    pub guidSubtype: GUID,
}

// ============= COM 接口定义 =============

// IMFActivate
#[repr(C)]
struct IMFActivate {
    vtable: *const IMFActivateVtbl,
}

#[repr(C)]
struct IMFActivateVtbl {
    query_interface: unsafe extern "system" fn(*mut IMFActivate, *const GUID, *mut *mut std::ffi::c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut IMFActivate) -> u32,
    release: unsafe extern "system" fn(*mut IMFActivate) -> u32,
}

// IMFAttributes
#[repr(C)]
struct IMFAttributes {
    vtable: *const IMFAttributesVtbl,
}

#[repr(C)]
struct IMFAttributesVtbl {
    // IUnknown
    query_interface: unsafe extern "system" fn(*mut IMFAttributes, *const GUID, *mut *mut std::ffi::c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut IMFAttributes) -> u32,
    release: unsafe extern "system" fn(*mut IMFAttributes) -> u32,

    // IMFAttributes methods
    get_item: usize,
    get_item_type: usize,
    compare_item: usize,
    compare: usize,
    get_uint32: usize,
    get_uint64: usize,
    get_double: usize,
    get_guid: usize,
    get_string_length: usize,
    get_string: usize,
    get_allocated_string: usize,
    get_blob_size: usize,
    get_blob: usize,
    get_allocatedblob: usize,
    get_unknown: usize,
    set_item: usize,
    delete_item: usize,
    delete_all_items: usize,
    set_uint32: unsafe extern "system" fn(*mut IMFAttributes, *const GUID, u32) -> i32,
    set_uint64: unsafe extern "system" fn(*mut IMFAttributes, *const GUID, u64) -> i32,
    set_double: usize,
    set_guid: usize,
    set_string: usize,
    set_blob: usize,
    set_unknown: usize,
    lock_store: usize,
    unlock_store: usize,
    get_count: usize,
    get_item_by_index: usize,
    copy_all_items: usize,
}

impl IMFAttributes {
    unsafe fn SetUINT32(&self, key: &GUID, value: u32) -> Result<()> {
        unsafe {
            HRESULT(((*self.vtable).set_uint32)(self as *const _ as *mut _, key, value)).ok()
        }
    }

    unsafe fn SetUINT64(&self, key: &GUID, value: u64) -> Result<()> {
        unsafe {
            HRESULT(((*self.vtable).set_uint64)(self as *const _ as *mut _, key, value)).ok()
        }
    }
}

// IMFMediaType
#[repr(C)]
struct IMFMediaType {
    vtable: *const IMFMediaTypeVtbl,
}

#[repr(C)]
struct IMFMediaTypeVtbl {
    // IUnknown
    query_interface: unsafe extern "system" fn(*mut IMFMediaType, *const GUID, *mut *mut std::ffi::c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut IMFMediaType) -> u32,
    release: unsafe extern "system" fn(*mut IMFMediaType) -> u32,

    // IMFAttributes methods (继承自 IMFAttributes)
    get_item: usize,
    get_item_type: usize,
    compare_item: usize,
    compare: usize,
    get_uint32: usize,
    get_uint64: usize,
    get_double: usize,
    get_guid: usize,
    get_string_length: usize,
    get_string: usize,
    get_allocated_string: usize,
    get_blob_size: usize,
    get_blob: usize,
    get_allocatedblob: usize,
    get_unknown: usize,
    set_item: usize,
    delete_item: usize,
    delete_all_items: usize,
    set_uint32: unsafe extern "system" fn(*mut IMFMediaType, *const GUID, u32) -> i32,
    set_uint64: unsafe extern "system" fn(*mut IMFMediaType, *const GUID, u64) -> i32,
    set_double: usize,
    set_guid: unsafe extern "system" fn(*mut IMFMediaType, *const GUID, *const GUID) -> i32,
    set_string: usize,
    set_blob: usize,
    set_unknown: usize,
    lock_store: usize,
    unlock_store: usize,
    get_count: usize,
    get_item_by_index: usize,
    copy_all_items: usize,

    // IMFMediaType methods
    get_major_type: usize,
    is_compressed_format: usize,
    is_equal: usize,
    get_representation: usize,
    free_representation: usize,
}

impl IMFMediaType {
    unsafe fn SetGUID(&self, key: &GUID, value: &GUID) -> Result<()> {
        unsafe {
            HRESULT(((*self.vtable).set_guid)(self as *const _ as *mut _, key, value)).ok()
        }
    }

    unsafe fn SetUINT32(&self, key: &GUID, value: u32) -> Result<()> {
        unsafe {
            HRESULT(((*self.vtable).set_uint32)(self as *const _ as *mut _, key, value)).ok()
        }
    }

    unsafe fn SetUINT64(&self, key: &GUID, value: u64) -> Result<()> {
        unsafe {
            HRESULT(((*self.vtable).set_uint64)(self as *const _ as *mut _, key, value)).ok()
        }
    }
}

// IMFSinkWriter
#[repr(C)]
struct IMFSinkWriter {
    vtable: *const IMFSinkWriterVtbl,
}

#[repr(C)]
struct IMFSinkWriterVtbl {
    // IUnknown
    query_interface: unsafe extern "system" fn(*mut IMFSinkWriter, *const GUID, *mut *mut std::ffi::c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut IMFSinkWriter) -> u32,
    release: unsafe extern "system" fn(*mut IMFSinkWriter) -> u32,

    // IMFSinkWriter methods
    add_stream: unsafe extern "system" fn(*mut IMFSinkWriter, *const IMFMediaType, *mut u32) -> i32,
    set_input_media_type: unsafe extern "system" fn(*mut IMFSinkWriter, u32, *const IMFMediaType, *const IMFAttributes) -> i32,
    begin_writing: unsafe extern "system" fn(*mut IMFSinkWriter) -> i32,
    write_sample: usize,
    send_stream_tick: usize,
    place_marker: usize,
    notify_end_of_segment: usize,
    flush: usize,
    finalize: usize,
    get_service_for_stream: usize,
    get_statistics: usize,
}

impl IMFSinkWriter {
    unsafe fn AddStream(&self, media_type: &IMFMediaType) -> Result<u32> {
        unsafe {
            let mut stream_index = 0u32;
            HRESULT(((*self.vtable).add_stream)(
                self as *const _ as *mut _,
                media_type,
                &mut stream_index
            )).ok()?;
            Ok(stream_index)
        }
    }

    unsafe fn SetInputMediaType(&self, stream_index: u32, media_type: &IMFMediaType, encoding_params: Option<&IMFAttributes>) -> Result<()> {
        unsafe {
            let params_ptr = encoding_params.map_or(std::ptr::null(), |p| p as *const _);
            HRESULT(((*self.vtable).set_input_media_type)(
                self as *const _ as *mut _,
                stream_index,
                media_type,
                params_ptr
            )).ok()
        }
    }

    unsafe fn BeginWriting(&self) -> Result<()> {
        unsafe {
            HRESULT(((*self.vtable).begin_writing)(self as *const _ as *mut _)).ok()
        }
    }
}

// ============= 辅助函数 =============

fn err_fail() -> anyhow::Error {
    anyhow::anyhow!("HRESULT 0x80004005 (E_FAIL)")
}

// ============= 主要逻辑 =============

pub async fn check_wmf_hardware(env: Arc<Mutex<Environment>>) {
    let has_hw_encoder = tokio::task::spawn_blocking(|| {
        check_hardware_authenticity()
    })
        .await
        .unwrap_or(false);

    let mut env_lock = env.lock().await;

    if has_hw_encoder {
        env_lock.add(action!(
            TrustType::PhysicalDevices,
            ScoreType::Gpu,
            s!("[wmf] Hardware GPU Video Encoder (H.264) detected via Media Foundation. Highly likely a physical device."),
            6,
            0.5
        ));
    } else {
        env_lock.add(action!(
            AbnormalType::Hardware,
            ScoreType::Gpu,
            s!("[wmf] No hardware video encoder available (Software rendering only). Likely a VM or Sandbox."),
            8,
            0.7
        ));
    }
}

pub fn check_hardware_authenticity() -> bool {
    unsafe { check_hardware_authenticity_inner() }
}

unsafe fn check_hardware_authenticity_inner() -> bool {
    let _guard = match unsafe { MfSession::new() } {
        Ok(g) => g,
        Err(_) => return false,
    };

    let has_hw_encoder = unsafe { check_hw_encoders() }.map(|count| count > 0).unwrap_or(false);
    if !has_hw_encoder {
        return false;
    }

    unsafe { try_hw_h264_pipeline() }.unwrap_or(false)
}

struct MfSession {
    com_initialized: bool,
    mf_started: bool,
}

impl MfSession {
    unsafe fn new() -> Result<Self> {
        unsafe {
            let co_init = resolve::<win_fn::FnCoInitializeEx>(ss!("ole32.dll"), ss!("CoInitializeEx")).ok_or_else(err_fail)?;
            let mf_startup = resolve::<win_fn::FnMFStartup>(ss!("mfplat.dll"), ss!("MFStartup")).ok_or_else(err_fail)?;

            HRESULT(co_init(std::ptr::null(), COINIT_MULTITHREADED as u32)).ok()?;

            match HRESULT(mf_startup(MF_VERSION, MFSTARTUP_FULL)).ok() {
                Ok(_) => Ok(Self { com_initialized: true, mf_started: true }),
                Err(e) => {
                    if let Some(co_uninit) = resolve::<win_fn::FnCoUninitialize>(ss!("ole32.dll"), ss!("CoUninitialize")) {
                        co_uninit();
                    }
                    Err(e)
                }
            }
        }
    }
}

impl Drop for MfSession {
    fn drop(&mut self) {
        unsafe {
            if self.mf_started {
                if let Some(mf_shutdown) = resolve::<win_fn::FnMFShutdown>(ss!("mfplat.dll"), ss!("MFShutdown")) {
                    let _ = mf_shutdown();
                }
            }
            if self.com_initialized {
                if let Some(co_uninit) = resolve::<win_fn::FnCoUninitialize>(ss!("ole32.dll"), ss!("CoUninitialize")) {
                    co_uninit();
                }
            }
        }
    }
}

unsafe fn check_hw_encoders() -> Result<u32> {
    unsafe {
        let mut count: u32 = 0;
        let mut activates: *mut *mut IMFActivate = std::ptr::null_mut();

        let output_info = MFT_REGISTER_TYPE_INFO {
            guidMajorType: MFMediaType_Video,
            guidSubtype: MFVideoFormat_H264,
        };

        let mft_enum_ex = resolve::<win_fn::FnMFTEnumEx>(ss!("mfplat.dll"), ss!("MFTEnumEx")).ok_or_else(err_fail)?;

        let hr = mft_enum_ex(
            MFT_CATEGORY_VIDEO_ENCODER,
            MFT_ENUM_FLAG_HARDWARE,
            std::ptr::null(),
            &output_info as *const _ as *const std::ffi::c_void,
            &mut activates as *mut _ as *mut *mut *mut std::ffi::c_void,
            &mut count,
        );

        let res = HRESULT(hr).ok();

        if res.is_ok() && !activates.is_null() {
            for i in 0..count as usize {
                let activate = *activates.add(i);
                if !activate.is_null() {
                    ((*(*activate).vtable).release)(activate);
                }
            }
            if let Some(co_task_mem_free) = resolve::<win_fn::FnCoTaskMemFree>(ss!("ole32.dll"), ss!("CoTaskMemFree")) {
                co_task_mem_free(activates as *mut std::ffi::c_void);
            }
        }
        res.map(|_| count)
    }
}

unsafe fn try_hw_h264_pipeline() -> Result<bool> {
    unsafe {
        let mf_create_attrs = resolve::<win_fn::FnMFCreateAttributes>(ss!("mfplat.dll"), ss!("MFCreateAttributes")).ok_or_else(err_fail)?;
        let mf_create_sink_writer = resolve::<win_fn::FnMFCreateSinkWriterFromURL>(ss!("mfreadwrite.dll"), ss!("MFCreateSinkWriterFromURL")).ok_or_else(err_fail)?;

        let mut attributes: *mut IMFAttributes = std::ptr::null_mut();
        HRESULT(mf_create_attrs(&mut attributes as *mut _ as *mut *mut std::ffi::c_void, 1)).ok()?;

        if attributes.is_null() {
            return Ok(false);
        }

        (*attributes).SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;

        let path_to_use = pick_probe_path();
        let _delete_probe = FileDeleteGuard::new(path_to_use.clone());

        let output_path: Vec<u16> = path_to_use.encode_utf16().chain(Some(0)).collect();

        let mut writer: *mut IMFSinkWriter = std::ptr::null_mut();

        let hr = mf_create_sink_writer(
            output_path.as_ptr(),
            std::ptr::null_mut(),
            attributes as *mut std::ffi::c_void,
            &mut writer as *mut _ as *mut *mut std::ffi::c_void
        );

        ((*(*attributes).vtable).release)(attributes);

        HRESULT(hr).ok()?;

        if writer.is_null() {
            return Ok(false);
        }

        let output_mt = create_h264_type()?;
        let stream_idx = (*writer).AddStream(&*output_mt)?;

        let input_mt = create_input_type()?;
        let _ = (*writer).SetInputMediaType(stream_idx, &*input_mt, None);

        let result = (*writer).BeginWriting().is_ok();

        ((*(*writer).vtable).release)(writer);
        ((*(*output_mt).vtable).release)(output_mt as *mut _);
        ((*(*input_mt).vtable).release)(input_mt as *mut _);

        Ok(result)
    }
}

fn pick_probe_path() -> String {
    let preferred = sss!("C:\\Windows\\Temp\\hw_probe.mp4");
    if fs::write(&preferred, b"").is_ok() {
        let _ = fs::remove_file(&preferred);
        return preferred;
    }
    std::env::temp_dir().join(ss!("hw_probe.mp4")).to_string_lossy().to_string()
}

struct FileDeleteGuard { path: String }
impl FileDeleteGuard { fn new(path: String) -> Self { Self { path } } }
impl Drop for FileDeleteGuard {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.path); }
}

unsafe fn create_h264_type() -> Result<*mut IMFMediaType> {
    unsafe {
        let mf_create_media_type = resolve::<win_fn::FnMFCreateMediaType>(ss!("mfplat.dll"), ss!("MFCreateMediaType")).ok_or_else(err_fail)?;
        let mut mt: *mut IMFMediaType = std::ptr::null_mut();

        HRESULT(mf_create_media_type(&mut mt as *mut _ as *mut *mut std::ffi::c_void)).ok()?;

        if mt.is_null() {
            return Err(err_fail());
        }

        (*mt).SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        (*mt).SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        (*mt).SetUINT32(&MF_MT_AVG_BITRATE, 4_000_000)?;
        (*mt).SetUINT64(&MF_MT_FRAME_SIZE, ((1280u64) << 32) | 720u64)?;
        (*mt).SetUINT64(&MF_MT_FRAME_RATE, ((30u64) << 32) | 1u64)?;
        (*mt).SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;

        Ok(mt)
    }
}

unsafe fn create_input_type() -> Result<*mut IMFMediaType> {
    unsafe {
        let mf_create_media_type = resolve::<win_fn::FnMFCreateMediaType>(ss!("mfplat.dll"), ss!("MFCreateMediaType")).ok_or_else(err_fail)?;
        let mut mt: *mut IMFMediaType = std::ptr::null_mut();

        HRESULT(mf_create_media_type(&mut mt as *mut _ as *mut *mut std::ffi::c_void)).ok()?;

        if mt.is_null() {
            return Err(err_fail());
        }

        (*mt).SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        (*mt).SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
        (*mt).SetUINT64(&MF_MT_FRAME_SIZE, ((1280u64) << 32) | 720u64)?;

        Ok(mt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test() {
        let env = Environment::new();
        check_wmf_hardware(env.clone()).await;
        sprint!(env.lock().await.dump_report());
    }
}