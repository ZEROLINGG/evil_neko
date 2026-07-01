//lib/proc-macro/src/obf/rt/image.rs
#![allow(unused_qualifications)]
#![allow(clippy::similar_names)]
#![allow(unused)]
#![cfg(feature = "image")]

/// RGBA 映射表模板: (pixel_idx, channel_idx)
const RGBA_CHANNEL_TABLE: [(u8, u8); 8] = [
    (0, 0), (0, 1), (0, 2), (0, 3),
    (1, 0), (1, 1), (1, 2), (1, 3),
];

/// RGB 映射表模板: (pixel_idx, bit_pos, channel_idx)
const RGB_CHANNEL_TABLE: [(u8, u8, u8); 12] = [
    (0, 0, 0), (1, 0, 0), (0, 0, 1), (1, 0, 1), (0, 0, 2), (1, 0, 2),
    (0, 1, 0), (1, 1, 0), (0, 1, 1), (1, 1, 1), (0, 1, 2), (1, 1, 2),
];




#[inline(always)]
pub fn __inject_byte_into_rgba_pixel_pair(
    pixel_pair: (&mut image::Rgba<u8>, &mut image::Rgba<u8>),
    byte: u8,
    key: u32,
) {
    let mut table = RGBA_CHANNEL_TABLE;

    unsafe {
        __random_table(table.as_mut_ptr(), table.len(), key);
    }

    let px0 = pixel_pair.0;
    let px1 = pixel_pair.1;

    for i in 0..8 {
        let (pixel_idx, channel_idx) = table[i];

        let bit_val = (byte >> i) & 1;

        let pixel = if pixel_idx == 0 { &mut *px0 } else { &mut *px1 };
        let channel = &mut pixel[channel_idx as usize];

        // 仅修改最低 1 位 (0xFE = 11111110)
        *channel = (*channel & 0xFE) | bit_val;
    }
}

#[inline(always)]
pub fn __extract_byte_from_rgba_pixel_pair(
    pixel_pair: (&image::Rgba<u8>, &image::Rgba<u8>),
    key: u32,
) -> u8 {
    let mut table = RGBA_CHANNEL_TABLE;


    unsafe {
        __random_table(table.as_mut_ptr(), table.len(), key);
    }

    let mut byte = 0u8;
    let px0 = pixel_pair.0;
    let px1 = pixel_pair.1;

    for i in 0..8 {
        let (pixel_idx, channel_idx) = table[i];

        let pixel = if pixel_idx == 0 { px0 } else { px1 };
        let channel = pixel[channel_idx as usize];

        // 提取最低 1 位
        let bit_val = channel & 1;

        byte |= bit_val << i;
    }

    byte
}

/// 核心注入逻辑 (RGBA - 支持交错写入与Seed流密钥)
#[inline(never)]
#[cold]
pub fn rgba_lsb_inject(img: &mut image::RgbaImage, payload: &[u8], stride: usize, seed: u128) -> bool {
    if stride == 0 {
        return false;
    }

    let width = img.width() as usize;
    let height = img.height() as usize;
    let total_pixels = width * height;

    // 两个像素存一字节
    let total_pairs = total_pixels / 2;

    let payload_len = payload.len();
    let total_bytes_needed = 4 + payload_len;

    if total_bytes_needed > total_pairs {
        return false;
    }

    let mut s = next_u128(seed);

    let len_bytes = (payload_len as u32).to_le_bytes();
    let bytes_to_write = len_bytes.iter().chain(payload.iter());

    let indices = (0..stride).flat_map(|offset| {
        (offset..total_pairs).step_by(stride)
    });

    let ptr = img.as_mut().as_mut_ptr() as *mut image::Rgba<u8>;

    for (pair_idx, &byte) in indices.zip(bytes_to_write) {
        s = next_u128(s);
        let key = s as u32;

        let idx0 = pair_idx * 2;
        let idx1 = pair_idx * 2 + 1;

        unsafe {
            let px0 = &mut *ptr.add(idx0);
            let px1 = &mut *ptr.add(idx1);

            __inject_byte_into_rgba_pixel_pair((px0, px1), byte, key);
        }
    }

    true
}

/// 核心提取逻辑 (RGBA - 支持交错读取与Seed流密钥)
#[inline(never)]
#[cold]
pub fn rgba_lsb_extract(img: &image::RgbaImage, stride: usize, seed: u128) -> Option<Vec<u8>> {
    if stride == 0 {
        return None;
    }

    let width = img.width() as usize;
    let height = img.height() as usize;
    let total_pixels = width * height;
    let total_pairs = total_pixels / 2;

    let mut s = next_u128(seed);

    let mut indices = (0..stride).flat_map(|offset| {
        (offset..total_pairs).step_by(stride)
    });

    let ptr = img.as_ref().as_ptr() as *const image::Rgba<u8>;

    let mut read_next_byte = || -> Option<u8> {
        let pair_idx = indices.next()?;

        s = next_u128(s);
        let key = s as u32;

        let idx0 = pair_idx * 2;
        let idx1 = pair_idx * 2 + 1;

        unsafe {
            let px0 = &*ptr.add(idx0);
            let px1 = &*ptr.add(idx1);
            Some(__extract_byte_from_rgba_pixel_pair((px0, px1), key))
        }
    };

    let mut len_bytes = [0u8; 4];
    for i in 0..4 {
        len_bytes[i] = read_next_byte()?;
    }

    let payload_len = u32::from_le_bytes(len_bytes) as usize;

    if payload_len > total_pairs.saturating_sub(4) {
        return None;
    }

    let mut payload = Vec::with_capacity(payload_len);
    for _ in 0..payload_len {
        payload.push(read_next_byte()?);
    }

    Some(payload)
}



#[inline(always)]
pub fn __inject_byte_into_rgb_pixel_pair(
    pixel_pair: (&mut image::Rgb<u8>, &mut image::Rgb<u8>),
    byte: u8,
    key: u32,
) {
    let mut table = RGB_CHANNEL_TABLE;


    unsafe {
        __random_table(table.as_mut_ptr(), table.len(), key);
    }

    let px0 = pixel_pair.0;
    let px1 = pixel_pair.1;

    for i in 0..8 {
        let (pixel_idx, bit_pos, channel_idx) = table[i];

        let bit_val = (byte >> i) & 1;

        let pixel = if pixel_idx == 0 { &mut *px0 } else { &mut *px1 };

        let channel = &mut pixel[channel_idx as usize];

        let mask = !(1 << bit_pos);
        *channel = (*channel & mask) | (bit_val << bit_pos);
    }
}

#[inline(always)]
pub fn __extract_byte_from_rgb_pixel_pair(
    pixel_pair: (&image::Rgb<u8>, &image::Rgb<u8>),
    key: u32,
) -> u8 {
    let mut table = RGB_CHANNEL_TABLE;


    unsafe {
        __random_table(table.as_mut_ptr(), table.len(), key);
    }

    let mut byte = 0u8;
    let px0 = pixel_pair.0;
    let px1 = pixel_pair.1;

    for i in 0..8 {
        let (pixel_idx, bit_pos, channel_idx) = table[i];

        let pixel = if pixel_idx == 0 { px0 } else { px1 };
        let channel = pixel[channel_idx as usize];

        let bit_val = (channel >> bit_pos) & 1;

        byte |= bit_val << i;
    }

    byte
}
#[inline(never)]
#[cold]
pub fn rgb_lsb_inject(img: &mut image::RgbImage, payload: &[u8], stride: usize, seed: u128) -> bool {
    if stride == 0 {
        return false;
    }

    let width = img.width() as usize;
    let height = img.height() as usize;
    let total_pixels = width * height;

    let total_pairs = total_pixels / 2;

    let payload_len = payload.len();
    let total_bytes_needed = 4 + payload_len;

    if total_bytes_needed > total_pairs {
        return false;
    }

    let mut s = next_u128(seed);

    let len_bytes = (payload_len as u32).to_le_bytes();
    let bytes_to_write = len_bytes.iter().chain(payload.iter());

    let indices = (0..stride).flat_map(|offset| {
        (offset..total_pairs).step_by(stride)
    });

    let ptr = img.as_mut().as_mut_ptr() as *mut image::Rgb<u8>;

    for (pair_idx, &byte) in indices.zip(bytes_to_write) {
        s = next_u128(s);
        let key = s as u32;

        let idx0 = pair_idx * 2;
        let idx1 = pair_idx * 2 + 1;

        unsafe {
            let px0 = &mut *ptr.add(idx0);
            let px1 = &mut *ptr.add(idx1);

            __inject_byte_into_rgb_pixel_pair((px0, px1), byte, key);
        }
    }

    true
}
#[inline(never)]
#[cold]
pub fn rgb_lsb_extract(img: &image::RgbImage, stride: usize, seed: u128) -> Option<Vec<u8>> {
    if stride == 0 {
        return None;
    }

    let width = img.width() as usize;
    let height = img.height() as usize;
    let total_pixels = width * height;
    let total_pairs = total_pixels / 2;

    let mut s = next_u128(seed);

    let mut indices = (0..stride).flat_map(|offset| {
        (offset..total_pairs).step_by(stride)
    });

    let ptr = img.as_ref().as_ptr() as *const image::Rgb<u8>;

    let mut read_next_byte = || -> Option<u8> {
        let pair_idx = indices.next()?;

        s = next_u128(s);
        let key = s as u32;

        let idx0 = pair_idx * 2;
        let idx1 = pair_idx * 2 + 1;

        unsafe {
            let px0 = &*ptr.add(idx0);
            let px1 = &*ptr.add(idx1);
            Some(__extract_byte_from_rgb_pixel_pair((px0, px1), key))
        }
    };

    let mut len_bytes = [0u8; 4];
    for i in 0..4 {
        len_bytes[i] = read_next_byte()?;
    }

    let payload_len = u32::from_le_bytes(len_bytes) as usize;

    if payload_len > total_pairs.saturating_sub(4) {
        return None;
    }

    let mut payload = Vec::with_capacity(payload_len);
    for _ in 0..payload_len {
        payload.push(read_next_byte()?);
    }

    Some(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    // ---------- 工具函数 ----------

    fn make_rgba(w: u32, h: u32, fill: u8) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba([fill, fill, fill, 255]))
    }

    fn make_rgb(w: u32, h: u32, fill: u8) -> RgbImage {
        RgbImage::from_pixel(w, h, Rgb([fill, fill, fill]))
    }

    // ---------- RGBA 像素对注入-提取 ----------

    #[test]
    fn rgba_pixel_pair_roundtrip_all_keys() {
        for key in [0, 42, 128, 255] {
            for byte in [0u8, 1, 0b10101010, 0b01010101, 255] {
                let mut px0 = Rgba([100, 100, 100, 255]);
                let mut px1 = Rgba([100, 100, 100, 255]);

                __inject_byte_into_rgba_pixel_pair((&mut px0, &mut px1), byte, key);
                let out = __extract_byte_from_rgba_pixel_pair((&px0, &px1), key);

                assert_eq!(out, byte, "key={key} byte={byte} roundtrip 失败");
            }
        }
    }

    #[test]
    fn rgba_inject_only_touches_lowest_bit() {
        let mut px0 = Rgba([0b1111_1111u8, 0b1010_1010, 0b0101_0101, 0b1100_1100]);
        let mut px1 = px0.clone();

        __inject_byte_into_rgba_pixel_pair((&mut px0, &mut px1), 0b0000_0000, 0);

        for ch in 0..4 {
            // 验证高 7 位未变 (mask: 0xFE = 11111110)
            assert_eq!(px0[ch] & 0xFE, [0b1111_1111u8, 0b1010_1010, 0b0101_0101, 0b1100_1100][ch] & 0xFE);
            assert_eq!(px1[ch] & 0xFE, [0b1111_1111u8, 0b1010_1010, 0b0101_0101, 0b1100_1100][ch] & 0xFE);
            // 验证最低位均为 0 (因为注入的 byte 全是 0)
            assert_eq!(px0[ch] & 1, 0);
            assert_eq!(px1[ch] & 1, 0);
        }
    }

    // ---------- RGB 像素对注入-提取 ----------

    #[test]
    fn rgb_pixel_pair_roundtrip_all_keys_and_bytes() {
        for key in [0u32, 1, 17, 128, 255] {
            for byte in [0u8, 1, 0b10101010, 0b01010101, 255] {
                let mut px0 = Rgb([0u8, 0, 0]);
                let mut px1 = Rgb([0u8, 0, 0]);
                __inject_byte_into_rgb_pixel_pair((&mut px0, &mut px1), byte, key);
                let out = __extract_byte_from_rgb_pixel_pair((&px0, &px1), key);
                assert_eq!(out, byte, "key={key} byte={byte} roundtrip 失败");
            }
        }
    }

    #[test]
    fn rgb_pixel_pair_wrong_key_usually_corrupts() {
        let byte = 0b1011_0010u8;
        let mut px0 = Rgb([0u8, 0, 0]);
        let mut px1 = Rgb([0u8, 0, 0]);
        __inject_byte_into_rgb_pixel_pair((&mut px0, &mut px1), byte, 1);
        let wrong = __extract_byte_from_rgb_pixel_pair((&px0, &px1), 2);
        assert_ne!(wrong, byte);
    }

    // ---------- RGBA 图像级 round-trip ----------

    #[test]
    fn rgba_lsb_roundtrip_basic() {
        let mut img = make_rgba(16, 16, 128);
        let payload = b"hello world, this is a test payload!".to_vec();
        let seed = 0xABCDEF;
        assert!(rgba_lsb_inject(&mut img, &payload, 1, seed));
        let extracted = rgba_lsb_extract(&img, 1, seed).expect("extract 应成功");
        assert_eq!(extracted, payload);
    }

    #[test]
    fn rgba_lsb_roundtrip_with_stride_and_seed() {
        let mut img = make_rgba(32, 32, 200);
        let payload: Vec<u8> = (0u8..=50).collect();
        let seed = 123456789;
        for stride in [1usize, 2, 3, 5, 7] {
            let mut img_clone = img.clone();
            assert!(rgba_lsb_inject(&mut img_clone, &payload, stride, seed));
            let extracted = rgba_lsb_extract(&img_clone, stride, seed).expect("extract 应成功");
            assert_eq!(extracted, payload, "stride={stride} 时 roundtrip 失败");
        }
    }

    #[test]
    fn rgba_lsb_empty_payload_roundtrip() {
        let mut img = make_rgba(4, 4, 0); // 16 pixels = 8 pairs
        let payload: Vec<u8> = vec![];
        let seed = 42;
        assert!(rgba_lsb_inject(&mut img, &payload, 1, seed));
        let extracted = rgba_lsb_extract(&img, 1, seed).unwrap();
        assert_eq!(extracted, payload);
    }

    #[test]
    fn rgba_lsb_stride_zero_rejected() {
        let mut img = make_rgba(8, 8, 10);
        assert!(!rgba_lsb_inject(&mut img, b"x", 0, 1));
        assert!(rgba_lsb_extract(&img, 0, 1).is_none());
    }

    #[test]
    fn rgba_lsb_payload_too_large_rejected() {
        let mut img = make_rgba(2, 2, 10); // total_pixels = 4, pairs = 2
        let payload = vec![0u8; 1]; // 4 (len prefix) + 1 > 2
        assert!(!rgba_lsb_inject(&mut img, &payload, 1, 1));
    }

    #[test]
    fn rgba_lsb_exact_capacity_boundary() {
        // total_pairs 恰好等于 4 + payload.len()
        let img_w = 6u32;
        let img_h = 3u32; // total_pixels = 18, pairs = 9
        let payload = vec![7u8; 5]; // 4 + 5 = 9，刚好贴边
        let mut img = make_rgba(img_w, img_h, 0);
        assert!(rgba_lsb_inject(&mut img, &payload, 1, 100));
        let extracted = rgba_lsb_extract(&img, 1, 100).unwrap();
        assert_eq!(extracted, payload);
    }

    #[test]
    fn rgba_lsb_wrong_seed_fails() {
        let mut img = make_rgba(20, 20, 50);
        let payload = b"top secret data".to_vec();
        assert!(rgba_lsb_inject(&mut img, &payload, 1, 1111));

        let result = rgba_lsb_extract(&img, 1, 2222);
        if let Some(data) = result {
            assert_ne!(data, payload);
        }
    }

    // ---------- RGB 图像级 round-trip ----------

    #[test]
    fn rgb_lsb_roundtrip_basic() {
        let mut img = make_rgb(16, 16, 50);
        let payload = b"rgb steganography test payload".to_vec();
        let seed = 0xDEADBEEFu128;
        assert!(rgb_lsb_inject(&mut img, &payload, 1, seed));
        let extracted = rgb_lsb_extract(&img, 1, seed).expect("extract 应成功");
        assert_eq!(extracted, payload);
    }

    #[test]
    fn rgb_lsb_roundtrip_with_stride_and_seed_variants() {
        let payload: Vec<u8> = (0u8..40).collect();
        for seed in [0u128, 1, 42, u128::MAX] {
            for stride in [1usize, 3, 5] {
                let mut img = make_rgb(40, 40, 90);
                assert!(rgb_lsb_inject(&mut img, &payload, stride, seed));
                let extracted = rgb_lsb_extract(&img, stride, seed)
                    .unwrap_or_else(|| panic!("seed={seed} stride={stride} 提取失败"));
                assert_eq!(extracted, payload, "seed={seed} stride={stride} 数据不一致");
            }
        }
    }

    #[test]
    fn rgb_lsb_wrong_seed_fails_or_corrupts() {
        let mut img = make_rgb(20, 20, 30);
        let payload = b"secret".to_vec();
        assert!(rgb_lsb_inject(&mut img, &payload, 1, 12345));
        let result = rgb_lsb_extract(&img, 1, 54321);
        if let Some(data) = result {
            assert_ne!(data, payload, "错误 seed 却恰好提取出了正确数据");
        }
    }

    #[test]
    fn rgb_lsb_stride_zero_rejected() {
        let mut img = make_rgb(8, 8, 10);
        assert!(!rgb_lsb_inject(&mut img, b"x", 0, 1));
        assert!(rgb_lsb_extract(&img, 0, 1).is_none());
    }

    #[test]
    fn rgb_lsb_payload_too_large_rejected() {
        let x = u16::MAX;
        let mut img = make_rgb(4, 4, 10); // total_pixels=16, total_pairs=8
        let payload = vec![0u8; 10]; // 4+10=14 > 8
        assert!(!rgb_lsb_inject(&mut img, &payload, 1, 1));
    }
}

// 放在#[cfg(test)]后面，便于框架识别
include::clean_include!("src/obf/rt/base.rs");