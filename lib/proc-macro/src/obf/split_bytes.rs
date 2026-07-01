//lib/proc-macro/src/obf/split_bytes.rs
#![allow(unused)]
#![cfg(feature = "buf")]
use proc_macro2::{Literal as Literal2, Literal, TokenStream as TokenStream2, TokenStream};
use quote::{quote, TokenStreamExt};
use std::sync::{LazyLock, Mutex};
use include_dir::{include_dir, Dir};
use rand::prelude::*;
use crate::junk::random_junk;


#[derive(Clone)]
enum Image {
    RgbaImage((image::RgbaImage, usize)), // (图像，总像素)
    RgbImage((image::RgbImage, usize)),
}

impl Image {
    fn from(file: &[u8], ext: &str) -> Result<Self, String> {
        let dyn_img = image::load_from_memory(file)
            .map_err(|e| format!("Image decoding error: {}", e))?;

        // 计算总像素数 (width * height)
        let width = dyn_img.width() as usize;
        let height = dyn_img.height() as usize;
        let total_pixels = width * height;

        // 判断图像是否包含 Alpha 通道（透明度），从而动态决定变体
        if dyn_img.color().has_alpha() {
            Ok(Image::RgbaImage((dyn_img.into_rgba8(), total_pixels)))
        } else {
            Ok(Image::RgbImage((dyn_img.into_rgb8(), total_pixels)))
        }
    }
}
static IMAGE_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/feather");

// 使用双重 LazyLock：
// 1. 外层 LazyLock 只在第一次访问时构建 Vec。
// 2. 内层 LazyLock 只在第一次使用某张具体图片时才执行 decode（解析图像是重度 CPU 运算）。
static IMAGES_LAZY: LazyLock<Vec<LazyLock<Image, Box<dyn Fn() -> Image + Send + Sync>>>> = LazyLock::new(|| {
    let mut vec = Vec::new();

    for file in IMAGE_DIR.files() {
        let path = file.path();
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e,
            None => continue,
        };

        if !matches!(
            ext.to_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "webp" | "bmp"
        ) {
            continue;
        }

        let contents = file.contents();
        let ext_string = ext.to_string();
        let path_string = path.display().to_string();

        let init_closure: Box<dyn Fn() -> Image + Send + Sync> = Box::new(move || {
            Image::from(contents, &ext_string).unwrap_or_else(|e| {
                panic!("Failed to decode embedded Image {}: {}", path_string, e)
            })
        });

        vec.push(LazyLock::new(init_closure));
    }

    vec
});

fn random_image(use_rgb: bool, size: usize) -> Option<Image> {
    let mut images = Vec::new();
    for _ in 0..10 {
        let image = &**choose(&**IMAGES_LAZY).unwrap();
        match image {
            Image::RgbaImage(i) => {
                if !use_rgb {
                    if i.1 >= (size * 2) + 8 { images.push(image) }
                }
            }
            Image::RgbImage(i) => {
                if use_rgb {
                    if i.1 >= (size * 2) + 8 { images.push(image) }
                }
            }
        }
        if images.len() >= 4 { break; }
    }
    if images.is_empty() { return None; }
    images
        .into_iter()
        .min_by_key(|image| match image {
            Image::RgbaImage(i) => i.1,
            Image::RgbImage(i) => i.1,
        })
        .cloned()
}

static LOW_ENTROPY_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/low_entropy");
static LOW_ENTROPY_FILES: LazyLock<Vec<Vec<u8>>> = LazyLock::new(|| {
    LOW_ENTROPY_DIR
        .files()
        .map(|file| file.contents().to_vec())
        .collect()
});

pub fn ___low_entropy_ts(min_size: usize, section: &str) -> TokenStream2 {
    let mut ts = TokenStream2::new();
    let mut current_size = 0usize;

    let min_size = (min_size as f64 * random_range(0.8..2.0)) as usize;

    while current_size < min_size {
        let file_bytes = choose(&*LOW_ENTROPY_FILES).unwrap();
        let pad_len = file_bytes.len();
        let pad_lit = Literal2::byte_string(file_bytes);

        let pad_name = syn::Ident::new(
            &format!("_F_{:x}", random::<u64>()),
            proc_macro2::Span::call_site(),
        );

        ts.extend(quote! {{
            let x = |i: u8| {
                #[unsafe(link_section = #section)]
                static #pad_name: [u8; #pad_len] = *#pad_lit;
                #pad_name.as_ptr()
            };
            let y = ::std::hint::black_box(x(0));
        }});
        current_size += pad_len;
    }

    ts
}

type ObfuscatorFn = fn(&[u8], syn::Ident) -> TokenStream2;

static FN: LazyLock<Vec<(&'static str, ObfuscatorFn, u8)>> = LazyLock::new(|| {
    vec![
        // (函数名字符串, 函数指针, 权重值)
        (stringify!(__static_thread_local), __static_thread_local, 6),
        (stringify!(__static_other), __static_other, 5),
        (stringify!(__mac_address), __mac_address, 8),
        (stringify!(__md5), __md5, 2),
        (stringify!(__sha256), __sha256, 3),
        (stringify!(__ipv6), __ipv6, 5),
        (stringify!(__uuid), __uuid, 6),
        (stringify!(__u128_xor), __u128_xor, 10),
        (stringify!(__u64_xor), __u64_xor, 9),
        (stringify!(__u32_xor), __u32_xor, 7),
        (stringify!(__u16_xor), __u16_xor, 4),
        (stringify!(__png_lsb), __png_lsb, 6),
        (stringify!(__bmp_lsb), __bmp_lsb, 7),
    ]
});
pub fn random_fn() -> (&'static str, fn(&[u8], syn::Ident) -> TokenStream2, u8) {
    let items = &*FN;

    let total_weight: u32 = items.iter().map(|item| item.2 as u32).sum();

    if (total_weight as usize) < items.len()  {
        return *choose(items).unwrap();
    }

    let mut r = random_range(0..total_weight);

    for item in items {
        if r < item.2 as u32 {
            return *item;
        }
        r -= item.2 as u32;
    }

    *choose(items).unwrap()
}

static RNG: LazyLock<Mutex<SmallRng>> = LazyLock::new(|| Mutex::new(rand::make_rng()));

pub fn random<T>() -> T
where
    rand::distr::StandardUniform: Distribution<T>,
{
    let mut rng = RNG.lock().unwrap();
    rng.random::<T>()
}
pub fn random_range<T, R>(range: R) -> T
where
    T: rand::distr::uniform::SampleUniform,
    R: rand::distr::uniform::SampleRange<T>,
{
    let mut rng = RNG.lock().unwrap();
    rng.random_range(range)
}
pub fn choose<T>(choices: &[T]) -> Option<&T> {
    let mut rng = RNG.lock().unwrap();
    choices.choose(&mut *rng)
}
pub fn choose_mut<T>(choices: &mut [T]) -> Option<&mut T> {
    let mut rng = RNG.lock().unwrap();
    choices.choose_mut(&mut *rng)
}



// 尽可能不要挤在.rodata段！！！

#[inline]
pub fn fmtname(chunk: &[u8]) -> String {
    format!("__{}_{}", random::<u64>(), chunk.iter().take(4).map(|b| format!("{:02X}", b)).collect::<String>())
}

pub fn __bmp_lsb(
    chunk: &[u8],
    buffer_name: syn::Ident,
) -> TokenStream2 {
    let chunk_len = chunk.len();
    if chunk_len == 0 {
        return TokenStream2::new();
    }

    let stride = random_range(5..10);
    let seed = random::<u128>();

    let selected_image = match random_image(true, chunk_len) {
        Some(img) => img,
        None => return (random_fn().1)(chunk, buffer_name),
    };

    let mut modified_bmp_bytes = Vec::new();
    let inject_success;

    match selected_image {
        Image::RgbImage((mut img, _)) => {
            inject_success = crate::obf::rt::image::rgb_lsb_inject(&mut img, chunk, stride, seed);
            if inject_success {
                let mut cursor = std::io::Cursor::new(&mut modified_bmp_bytes);
                // 编码回 BMP 格式
                image::DynamicImage::ImageRgb8(img)
                    .write_to(&mut cursor, image::ImageFormat::Bmp)
                    .expect("Failed to encode obfuscated RGB image back to BMP");
            }
        }
        Image::RgbaImage(_) => {
            // 兜底处理：理论上 `random_image(true, ..)` 不会返回 RGBA
            return (random_fn().1)(chunk, buffer_name);
        }
    }

    if !inject_success {
        return (random_fn().1)(chunk, buffer_name);
    }

    let chunk_name = fmtname(chunk);
    let chunk_ident = syn::Ident::new(&chunk_name.to_uppercase(), proc_macro2::Span::call_site());

    let byte_lit = Literal2::byte_string(&modified_bmp_bytes);
    let bmp_len = modified_bmp_bytes.len();
    let stride_lit = Literal2::usize_unsuffixed(stride);

    let mut ts = TokenStream2::new();

    ts.extend(___low_entropy_ts(bmp_len, ".file"));

    let extract_code = quote! {
        let img = ::lib::image::load_from_memory_with_format(&#chunk_ident, ::lib::image::ImageFormat::Bmp)
            .unwrap()
            .into_rgb8();
        rgb_lsb_extract(&img, #stride_lit, #seed).unwrap()
    };

    ts.extend(quote! {
        #buffer_name.push((|| async {
            unsafe {
                #[unsafe(link_section = ".file")]
                static #chunk_ident: [u8; #bmp_len] = *#byte_lit;

                #extract_code
            }
        }, #chunk_len, ()));
    });

    ts
}

pub fn __png_lsb(
    chunk: &[u8],
    buffer_name: syn::Ident,
) -> TokenStream2 {
    let chunk_len = chunk.len();
    if chunk_len == 0 {
        return TokenStream2::new();
    }

    let stride = random_range(5..10);
    let seed = random::<u128>();
    let use_rgb = random::<bool>();

    let selected_image = match random_image(use_rgb, chunk_len) {
        Some(img) => img,
        None => match random_image(!use_rgb, chunk_len) {
            Some(img) => img,
            None => return (random_fn().1)(chunk, buffer_name)
        }
    };

    let mut modified_png_bytes = Vec::new();
    let inject_success;
    let is_rgb;

    match selected_image {
        Image::RgbaImage((mut img, _)) => {
            is_rgb = false;
            inject_success = crate::obf::rt::image::rgba_lsb_inject(&mut img, chunk, stride, seed);
            if inject_success {
                let mut cursor = std::io::Cursor::new(&mut modified_png_bytes);
                image::DynamicImage::ImageRgba8(img)
                    .write_to(&mut cursor, image::ImageFormat::Png)
                    .expect("Failed to encode obfuscated RGBA image back to PNG");
            }
        }
        Image::RgbImage((mut img, _)) => {
            is_rgb = true;
            inject_success = crate::obf::rt::image::rgb_lsb_inject(&mut img, chunk, stride, seed);
            if inject_success {
                let mut cursor = std::io::Cursor::new(&mut modified_png_bytes);
                image::DynamicImage::ImageRgb8(img)
                    .write_to(&mut cursor, image::ImageFormat::Png)
                    .expect("Failed to encode obfuscated RGB image back to PNG");
            }
        }
    }

    if !inject_success {
        return (random_fn().1)(chunk, buffer_name);
    }

    let chunk_name = fmtname(chunk);
    let chunk_ident = syn::Ident::new(&chunk_name.to_uppercase(), proc_macro2::Span::call_site());

    let byte_lit = Literal2::byte_string(&modified_png_bytes);
    let png_len = modified_png_bytes.len();
    let stride_lit = Literal2::usize_unsuffixed(stride);

    let mut ts = TokenStream2::new();
    ts.extend(___low_entropy_ts(png_len, ".file"));

    let extract_code = if is_rgb {
        quote! {
            let img = ::lib::image::load_from_memory_with_format(&#chunk_ident, ::lib::image::ImageFormat::Png)
                .unwrap()
                .into_rgb8();
            rgb_lsb_extract(&img, #stride_lit, #seed).unwrap()
        }
    } else {
        quote! {
            let img = ::lib::image::load_from_memory_with_format(&#chunk_ident, ::lib::image::ImageFormat::Png)
                .unwrap()
                .into_rgba8();
            rgba_lsb_extract(&img, #stride_lit, #seed).unwrap()
        }
    };

    // 运行时会自动注入
    ts.extend(quote! {
        #buffer_name.push((|| async {
            unsafe {
                #[unsafe(link_section = ".file")]
                static #chunk_ident: [u8; #png_len] = *#byte_lit;

                #extract_code
            }
        }, #chunk_len, ()));
    });

    ts
}


pub fn __static_thread_local(
    chunk: &[u8],
    buffer_name: syn::Ident,
) -> TokenStream2 {
    let chunk_len = chunk.len();
    if chunk_len == 0 {
        return TokenStream2::new();
    }

    let chunk_name = fmtname(chunk);
    let chunk_ident = syn::Ident::new(&chunk_name.to_uppercase(), proc_macro2::Span::call_site());
    let key_ident = syn::Ident::new(
        &format!("{}_K", chunk_name.to_uppercase()),
        proc_macro2::Span::call_site(),
    );

    let key_len = random_range(4..=8);
    let key: Vec<u8> = (0..key_len).map(|_| random::<u8>()).collect();

    let swap_bits = |b: u8| -> u8 {
        ((b & 0b1100_0000) >> 6) |
            ((b & 0b0011_0000) >> 2) |
            ((b & 0b0000_1100) << 2) |
            ((b & 0b0000_0011) << 6)
    };

    let obfuscated_chunk: Vec<u8> = chunk
        .iter()
        .enumerate()
        .map(|(idx, &b)| {
            let xored = b ^ key[idx % key_len];
            swap_bits(xored)
        })
        .collect();

    let byte_lit = Literal2::byte_string(&obfuscated_chunk);
    let key_lit = Literal2::byte_string(&key);

    let mut ts2 = TokenStream2::new();

    ts2.extend(quote! {
        ::std::thread_local! {
            static #chunk_ident: [u8; #chunk_len] = *#byte_lit;
        }

        #buffer_name.push((
            |idx: usize| async move {
                const #key_ident: &[u8] = #key_lit;

                #chunk_ident.with(|val| {
                    let b = val[idx];
                    let swapped = ((b & 0b1100_0000) >> 6) |
                                  ((b & 0b0011_0000) >> 2) |
                                  ((b & 0b0000_1100) << 2) |
                                  ((b & 0b0000_0011) << 6);
                    swapped ^ #key_ident[idx % #key_len]
                })
            },
            #chunk_len
        ));
    });

    ts2
}

pub fn __static_other(
    chunk: &[u8],
    buffer_name: syn::Ident,
) -> TokenStream2 {
    let chunk_len = chunk.len();
    if chunk_len == 0 {
        return TokenStream2::new();
    }

    let chunk_name = fmtname(chunk);
    let chunk_ident = syn::Ident::new(&chunk_name.to_uppercase(), proc_macro2::Span::call_site());

    let seed: u128 = random::<u128>();
    let mut stream_key = seed;

    let obfuscated_chunk: Vec<u8> = chunk
        .iter()
        .map(|&b| {
            // 推进密钥流
            stream_key = crate::obf::rt::base::next_u128(stream_key);
            let mask = stream_key as u8;

            let xored = b ^ mask;
            let swapped = (xored >> 4) | (xored << 4);
            swapped ^ 0x5A
        })
        .collect();

    let byte_lit = Literal2::byte_string(&obfuscated_chunk);
    let seed_lit = Literal2::u128_unsuffixed(seed);

    let mut ts2 = TokenStream2::new();

    let links = vec![
        ".data".to_string(),
        ".file".to_string(),
        format!(".file_{:08x}", random::<u32>()),
    ];
    let link = links.choose(&mut *RNG.lock().unwrap()).unwrap().to_string();

    ts2.extend(___low_entropy_ts(chunk_len, &link));

    ts2.extend(quote! {
        #buffer_name.push((|| async {
            #[unsafe(link_section = #link)]
            static #chunk_ident: [u8; #chunk_len] = *#byte_lit;

            let mut buf = #chunk_ident;
            let mut rt_key = ::std::hint::black_box(#seed_lit);

            for i in 0..#chunk_len {
                rt_key = next_u128(rt_key);
                let mask = ::std::hint::black_box(rt_key) as u8;

                let mut b = buf[i];

                b ^= 0x5A;
                b = (b >> 4) | (b << 4);
                b ^= mask;

                buf[i] = b;
            }
            buf.to_vec()
        }, #chunk_len, ()));
    });

    ts2
}

pub fn __u128_xor(
    chunk: &[u8],
    buffer_name: syn::Ident,
) -> TokenStream2 {
    let chunk_name = fmtname(chunk);
    let chunk_len = chunk.len();

    if chunk_len == 0 {
        return TokenStream2::new();
    }

    let prefix = chunk_name.to_uppercase();
    let full_chunks = chunk_len / 16;
    let tail_len = chunk_len % 16;

    let seed: u128 = random();
    let mut stream_key = seed;

    let mut u128_consts = TokenStream2::new();
    let mut restore_stmts = TokenStream2::new();

    for i in 0..full_chunks {
        let slice = &chunk[i * 16..(i + 1) * 16];
        let use_le: bool = random();

        let val = if use_le {
            u128::from_le_bytes(slice.try_into().unwrap())
        } else {
            u128::from_be_bytes(slice.try_into().unwrap())
        };

        stream_key = crate::obf::rt::base::next_u128(stream_key);
        let obf_val = val ^ stream_key;

        let const_ident = syn::Ident::new(
            &format!("{}_U128_{}", prefix, i),
            proc_macro2::Span::call_site(),
        );

        let start = i * 16;
        let end = start + 16;
        let obf_lit = Literal::u128_suffixed(obf_val);

        u128_consts.extend(quote! {
            const #const_ident: u128 = #obf_lit;
        });

        let method = if use_le { quote!(to_le_bytes) } else { quote!(to_be_bytes) };
        restore_stmts.extend(quote! {
            rt_key = next_u128(rt_key);
            buf[#start..#end].copy_from_slice(
                &(::std::hint::black_box(#const_ident) ^ rt_key).#method()
            );
        });
    }

    if tail_len > 0 {
        let tail_slice = &chunk[full_chunks * 16..];
        let mut padded = [0u8; 16];
        padded[..tail_len].copy_from_slice(tail_slice);

        let use_le: bool = random();
        let val = if use_le {
            u128::from_le_bytes(padded)
        } else {
            u128::from_be_bytes(padded)
        };

        stream_key = crate::obf::rt::base::next_u128(stream_key);
        let obf_val = val ^ stream_key;

        let const_ident = syn::Ident::new(
            &format!("{}_U128_TAIL", prefix),
            proc_macro2::Span::call_site(),
        );

        let start = full_chunks * 16;
        let obf_lit = Literal::u128_suffixed(obf_val);

        u128_consts.extend(quote! {
            const #const_ident: u128 = #obf_lit;
        });

        let method = if use_le { quote!(to_le_bytes) } else { quote!(to_be_bytes) };
        restore_stmts.extend(quote! {
            rt_key = next_u128(rt_key);
            buf[#start..#start + #tail_len].copy_from_slice(
                &(::std::hint::black_box(#const_ident) ^ rt_key).#method()[..#tail_len]
            );
        });
    }

    let seed_lit = Literal::u128_suffixed(seed);
    let mut ts2 = TokenStream2::new();

    ts2.extend(quote! {
        #buffer_name.push({
            #u128_consts
            let mut buf = ::std::vec![0u8; #chunk_len];
            let mut rt_key = #seed_lit;
            #restore_stmts
            buf
        });
    });
    ts2
}

pub fn __u64_xor(
    chunk: &[u8],
    buffer_name: syn::Ident,
) -> TokenStream2 {
    let chunk_len = chunk.len();

    if chunk_len == 0 {
        return TokenStream2::new();
    }

    let full_chunks = chunk_len / 8;
    let tail_len = chunk_len % 8;

    let seed: u128 = random::<u128>();
    let mut stream_key = seed;

    let mut restore_stmts = TokenStream2::new();

    for i in 0..full_chunks {
        let slice = &chunk[i * 8..(i + 1) * 8];
        let use_le: bool = random();

        let val = if use_le {
            u64::from_le_bytes(slice.try_into().unwrap())
        } else {
            u64::from_be_bytes(slice.try_into().unwrap())
        };

        stream_key = crate::obf::rt::base::next_u128(stream_key);
        let mask = stream_key as u64;

        let obf_val = val ^ mask;
        let obf_lit = Literal::u64_suffixed(obf_val);

        let start = i * 8;
        let end = start + 8;
        let method = if use_le { quote!(to_le_bytes) } else { quote!(to_be_bytes) };

        restore_stmts.extend(quote! {
            rt_key = next_u128(rt_key);
            buf[#start..#end].copy_from_slice(
                &(::std::hint::black_box(#obf_lit) ^ (rt_key as u64)).#method()
            );
        });
    }

    if tail_len > 0 {
        let tail_slice = &chunk[full_chunks * 8..];
        let mut padded = [0u8; 8];
        padded[..tail_len].copy_from_slice(tail_slice);

        let use_le: bool = random();
        let val = if use_le {
            u64::from_le_bytes(padded)
        } else {
            u64::from_be_bytes(padded)
        };

        stream_key = crate::obf::rt::base::next_u128(stream_key);
        let mask = stream_key as u64;

        let obf_val = val ^ mask;
        let obf_lit = Literal::u64_suffixed(obf_val);

        let start = full_chunks * 8;
        let method = if use_le { quote!(to_le_bytes) } else { quote!(to_be_bytes) };

        restore_stmts.extend(quote! {
            rt_key = next_u128(rt_key);
            buf[#start..#start + #tail_len].copy_from_slice(
                &(::std::hint::black_box(#obf_lit) ^ (rt_key as u64)).#method()[..#tail_len]
            );
        });
    }

    let seed_lit = Literal::u128_suffixed(seed);
    let mut ts2 = TokenStream2::new();

    ts2.extend(quote! {
        #buffer_name.push({
            let mut buf = ::std::vec![0u8; #chunk_len];
            let mut rt_key = #seed_lit;
            #restore_stmts
            buf
        });
    });

    ts2
}

pub fn __u32_xor(
    chunk: &[u8],
    buffer_name: syn::Ident,
) -> TokenStream2 {
    let chunk_len = chunk.len();

    if chunk_len == 0 {
        return TokenStream2::new();
    }

    // u32 是 4 字节 (32 bit)
    let full_chunks = chunk_len / 4;
    let tail_len = chunk_len % 4;

    let seed: u128 = random::<u128>();
    let mut stream_key = seed;

    let mut restore_stmts = TokenStream2::new();

    for i in 0..full_chunks {
        let slice = &chunk[i * 4..(i + 1) * 4];
        let use_le: bool = random();

        let val = if use_le {
            u32::from_le_bytes(slice.try_into().unwrap())
        } else {
            u32::from_be_bytes(slice.try_into().unwrap())
        };

        stream_key = crate::obf::rt::base::next_u128(stream_key);
        let mask = stream_key as u32;

        let obf_val = val ^ mask;
        let obf_lit = Literal::u32_suffixed(obf_val);

        let start = i * 4;
        let end = start + 4;

        let method = if use_le { quote!(to_le_bytes) } else { quote!(to_be_bytes) };

        restore_stmts.extend(quote! {
            rt_key = next_u128(rt_key);
            buf[#start..#end].copy_from_slice(
                &(::std::hint::black_box(#obf_lit) ^ (rt_key as u32)).#method()
            );
        });
    }

    if tail_len > 0 {
        let tail_slice = &chunk[full_chunks * 4..];
        let mut padded = [0u8; 4];
        padded[..tail_len].copy_from_slice(tail_slice);

        let use_le: bool = random();
        let val = if use_le {
            u32::from_le_bytes(padded)
        } else {
            u32::from_be_bytes(padded)
        };

        stream_key = crate::obf::rt::base::next_u128(stream_key);
        let mask = stream_key as u32;

        let obf_val = val ^ mask;
        let obf_lit = Literal::u32_suffixed(obf_val);

        let start = full_chunks * 4;
        let method = if use_le { quote!(to_le_bytes) } else { quote!(to_be_bytes) };

        restore_stmts.extend(quote! {
            rt_key = next_u128(rt_key);
            buf[#start..#start + #tail_len].copy_from_slice(
                &(::std::hint::black_box(#obf_lit) ^ (rt_key as u32)).#method()[..#tail_len]
            );
        });
    }

    let seed_lit = Literal::u128_suffixed(seed);
    let mut ts2 = TokenStream2::new();

    ts2.extend(quote! {
        #buffer_name.push({
            let mut buf = ::std::vec![0u8; #chunk_len];
            let mut rt_key = #seed_lit;

            #restore_stmts

            buf
        });
    });

    ts2
}

pub fn __u16_xor(
    chunk: &[u8],
    buffer_name: syn::Ident,
) -> TokenStream2 {
    let chunk_len = chunk.len();

    if chunk_len == 0 {
        return TokenStream2::new();
    }

    let full_chunks = chunk_len / 2;
    let tail_len = chunk_len % 2;

    let seed: u128 = random::<u128>();
    let mut stream_key = seed;

    let mut restore_stmts = TokenStream2::new();

    for i in 0..full_chunks {
        let slice = &chunk[i * 2..(i + 1) * 2];
        let use_le: bool = random();

        let val = if use_le {
            u16::from_le_bytes(slice.try_into().unwrap())
        } else {
            u16::from_be_bytes(slice.try_into().unwrap())
        };

        stream_key = crate::obf::rt::base::next_u128(stream_key);
        let mask = stream_key as u16;

        let obf_val = val ^ mask;
        let obf_lit = Literal::u16_suffixed(obf_val);

        let start = i * 2;
        let end = start + 2;

        let method = if use_le { quote!(to_le_bytes) } else { quote!(to_be_bytes) };

        restore_stmts.extend(quote! {
            rt_key = next_u128(rt_key);
            buf[#start..#end].copy_from_slice(
                &(::std::hint::black_box(#obf_lit) ^ (rt_key as u16)).#method()
            );
        });
    }

    if tail_len > 0 {
        let tail_slice = &chunk[full_chunks * 2..];
        let mut padded = [0u8; 2];
        padded[..tail_len].copy_from_slice(tail_slice);

        let use_le: bool = random();
        let val = if use_le {
            u16::from_le_bytes(padded)
        } else {
            u16::from_be_bytes(padded)
        };

        stream_key = crate::obf::rt::base::next_u128(stream_key);
        let mask = stream_key as u16;

        let obf_val = val ^ mask;
        let obf_lit = Literal::u16_suffixed(obf_val);

        let start = full_chunks * 2;
        let method = if use_le { quote!(to_le_bytes) } else { quote!(to_be_bytes) };

        restore_stmts.extend(quote! {
            rt_key = next_u128(rt_key);
            buf[#start..#start + #tail_len].copy_from_slice(
                &(::std::hint::black_box(#obf_lit) ^ (rt_key as u16)).#method()[..#tail_len]
            );
        });
    }

    let seed_lit = Literal::u128_suffixed(seed);
    let mut ts2 = TokenStream2::new();

    ts2.extend(quote! {
        #buffer_name.push({
            let mut buf = ::std::vec![0u8; #chunk_len];
            let mut rt_key = #seed_lit;
            #restore_stmts
            buf
        });
    });

    ts2
}

pub fn __mac_address(
    chunk: &[u8],
    buffer_name: syn::Ident,
) -> TokenStream2 {
    let chunk_len = chunk.len();

    if chunk_len == 0 {
        return TokenStream2::new();
    }

    let full_groups = chunk_len / 6;
    let tail_len = chunk_len % 6;

    // 编译期生成所有 MAC 字符串字面量，收集到一个数组
    // 同时记录每组对应的 buf 写入起始偏移 和 有效字节数（用于尾部截断）
    let total_groups = full_groups + if tail_len > 0 { 1 } else { 0 };

    let mut mac_strs: Vec<String> = Vec::with_capacity(total_groups);
    // sep_bytes[i]: 该组的分隔符字节，用于 black_box hint
    let mut sep_bytes: Vec<u8> = Vec::with_capacity(total_groups);
    // take_lens[i]: 该组实际写入 buf 的字节数（完整组=6，尾部组=tail_len）
    let mut take_lens: Vec<usize> = Vec::with_capacity(total_groups);
    // src_offsets[i][j]: 第 i 组第 j 个字节在 MAC 字符串中的起始字符下标
    // 由于分隔符位置固定（0,3,6,9,12,15），可以编译期算好
    // 但每组 sep 不同只影响字符串内容，偏移固定为 [0,3,6,9,12,15]
    // 直接内嵌为常量，无需per-group存储

    let emit_mac = |bytes6: &[u8; 6]| -> (String, u8) {
        let sep: char = if random::<bool>() { ':' } else { '-' };
        let upper: bool = random();
        let s = if upper {
            format!(
                "{:02X}{sep}{:02X}{sep}{:02X}{sep}{:02X}{sep}{:02X}{sep}{:02X}",
                bytes6[0], bytes6[1], bytes6[2], bytes6[3], bytes6[4], bytes6[5],
                sep = sep
            )
        } else {
            format!(
                "{:02x}{sep}{:02x}{sep}{:02x}{sep}{:02x}{sep}{:02x}{sep}{:02x}",
                bytes6[0], bytes6[1], bytes6[2], bytes6[3], bytes6[4], bytes6[5],
                sep = sep
            )
        };
        (s, sep as u8)
    };

    for i in 0..full_groups {
        let slice: [u8; 6] = chunk[i * 6..(i + 1) * 6].try_into().unwrap();
        let (s, sep_byte) = emit_mac(&slice);
        mac_strs.push(s);
        sep_bytes.push(sep_byte);
        take_lens.push(6);
    }

    if tail_len > 0 {
        let mut padded = [0u8; 6];
        padded[..tail_len].copy_from_slice(&chunk[full_groups * 6..]);
        for b in &mut padded[tail_len..] {
            *b = random();
        }
        let (s, sep_byte) = emit_mac(&padded);
        mac_strs.push(s);
        sep_bytes.push(sep_byte);
        take_lens.push(tail_len);
    }


    // 生成编译期常量数组 token
    // MACS: [&str; N]
    // SEPS: [u8; N]
    // TAKES: [usize; N]
    let n = total_groups;
    let mac_elems = mac_strs.iter().map(|s| quote! { #s, });
    let sep_elems = sep_bytes.iter().map(|b| quote! { #b, });
    let take_elems = take_lens.iter().map(|t| quote! { #t, });

    // 固定偏移：MAC字符串中第j个字节对应的hex起始位置
    // j=0->0, j=1->3, j=2->6, j=3->9, j=4->12, j=5->15


    let mut ts2 = TokenStream2::new();
    ts2.extend(___low_entropy_ts(chunk_len,".rodata"));

    ts2.extend(quote! {
        #buffer_name.push({
            let mut buf = ::std::vec![0u8; #chunk_len];

            // 编译期常量：所有 MAC 字符串、分隔符、有效字节数
            const MACS:  [&str;  #n] = [ #(#mac_elems)*  ];
            const SEPS:  [u8;    #n] = [ #(#sep_elems)*  ];
            const TAKES: [usize; #n] = [ #(#take_elems)* ];

            let mut group = 0usize;
            while group < #n {
                let mac   = ::std::hint::black_box(MACS[group]);
                let bytes = mac.as_bytes();
                let take  = TAKES[group];
                let _sep  = ::std::hint::black_box(SEPS[group]);

                let mut j = 0usize;
                while j < take {
                    let off = j * 3;
                    let hi  = (bytes[off]     as char).to_digit(16).unwrap() as u8;
                    let lo  = (bytes[off + 1] as char).to_digit(16).unwrap() as u8;
                    buf[group * 6 + j] = (hi << 4) | lo;
                    j += 1;
                }
                group += 1;
            }
            buf
        });
    });
    ts2
}

pub fn __md5(
    chunk: &[u8],
    buffer_name: syn::Ident,
) -> TokenStream2 {
    // 将字节序列伪装成 MD5 哈希风格字符串

    let chunk_len = chunk.len();

    if chunk_len == 0 {
        return TokenStream2::new();
    }

    let full_groups = chunk_len / 16;
    let tail_len = chunk_len % 16;

    let mut decode_stmts = TokenStream2::new();

    // 解码一组 hex 字符串到 buf[start..start+take]，padded_len 是字符串实际编码的字节数
    // 生成通用的解码块
    let emit_decode = |group_bytes: &[u8; 16],
                       start: usize,
                       take: usize,
                       decode_stmts: &mut TokenStream2| {
        let upper: bool = random();
        let hex_str = if upper {
            group_bytes
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<String>()
        } else {
            group_bytes
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>()
        };

        // 解码：每两个 ASCII 字符 -> 一个字节
        decode_stmts.extend(quote! {
            {
                let s = ::std::hint::black_box(#hex_str);
                let b = s.as_bytes();
                let mut i = 0usize;
                while i < #take {
                    let hi = (b[i * 2]     as char).to_digit(16).unwrap() as u8;
                    let lo = (b[i * 2 + 1] as char).to_digit(16).unwrap() as u8;
                    buf[#start + i] = (hi << 4) | lo;
                    i += 1;
                }
            }
        });
    };

    for i in 0..full_groups {
        let slice: [u8; 16] = chunk[i * 16..(i + 1) * 16].try_into().unwrap();
        emit_decode(&slice, i * 16, 16, &mut decode_stmts);
    }

    if tail_len > 0 {
        let mut padded = [0u8; 16];
        padded[..tail_len].copy_from_slice(&chunk[full_groups * 16..]);
        for b in &mut padded[tail_len..] {
            *b = random();
        }
        emit_decode(
            &padded,
            full_groups * 16,
            tail_len,
            &mut decode_stmts,
        );
    }


    let mut ts2 = TokenStream2::new();
    ts2.extend(___low_entropy_ts(chunk_len,".rodata"));

    ts2.extend(quote! {
        #buffer_name.push({
            let mut buf = ::std::vec![0u8; #chunk_len];
            #decode_stmts
            buf
        });
    });
    ts2
}

pub fn __sha256(
    chunk: &[u8],
    buffer_name: syn::Ident,
) -> TokenStream2 {
    // 将字节序列伪装成 SHA-256 哈希风格字符串

    let chunk_len = chunk.len();

    if chunk_len == 0 {
        return TokenStream2::new();
    }


    let full_groups = chunk_len / 32;
    let tail_len = chunk_len % 32;

    let mut decode_stmts = TokenStream2::new();

    let emit_decode = |
                       group_bytes: &[u8; 32],
                       start: usize,
                       take: usize,
                       decode_stmts: &mut TokenStream2| {
        let upper: bool = random();

        // 随机决定是否在第 8 字节后插入一个 '-'（模拟 "abcdef01-234..." 格式）
        // 插入后字符串长 65，解码时需跳过该字符
        let insert_dash: bool = random();
        let dash_pos: usize = if insert_dash {
            // dash 插入在第 N 个十六进制对之后，N ∈ [4, 28]
            random_range(4..28) * 2
        } else {
            usize::MAX // 哨兵值，表示不插入
        };

        let hex_chars: Vec<String> = group_bytes
            .iter()
            .map(|b| {
                if upper {
                    format!("{:02X}", b)
                } else {
                    format!("{:02x}", b)
                }
            })
            .collect();

        // 拼接，在 dash_pos 处插入 '-'
        let mut hex_str = String::with_capacity(65);
        for (ci, pair) in hex_chars.iter().enumerate() {
            let char_pos = ci * 2;
            if insert_dash && char_pos == dash_pos {
                hex_str.push('-');
            }
            hex_str.push_str(pair);
        }

        if insert_dash {
            // 解码时需跳过 '-'：先把字符串过滤为纯 hex 再解析
            decode_stmts.extend(quote! {
                {
                    let s = ::std::hint::black_box(#hex_str);
                    // 过滤掉非 hex 字符（即 '-'），收集为连续字节序列
                    let mut clean = [0u8; 64];
                    let mut ci = 0usize;
                    for &ch in s.as_bytes() {
                        if ch != b'-' {
                            clean[ci] = ch;
                            ci += 1;
                        }
                    }
                    let mut i = 0usize;
                    while i < #take {
                        let hi = (clean[i * 2]     as char).to_digit(16).unwrap() as u8;
                        let lo = (clean[i * 2 + 1] as char).to_digit(16).unwrap() as u8;
                        buf[#start + i] = (hi << 4) | lo;
                        i += 1;
                    }
                }
            });
        } else {
            decode_stmts.extend(quote! {
                {
                    let s = ::std::hint::black_box(#hex_str);
                    let b = s.as_bytes();
                    let mut i = 0usize;
                    while i < #take {
                        let hi = (b[i * 2]     as char).to_digit(16).unwrap() as u8;
                        let lo = (b[i * 2 + 1] as char).to_digit(16).unwrap() as u8;
                        buf[#start + i] = (hi << 4) | lo;
                        i += 1;
                    }
                }
            });
        }
    };

    for i in 0..full_groups {
        let slice: [u8; 32] = chunk[i * 32..(i + 1) * 32].try_into().unwrap();
        emit_decode(&slice, i * 32, 32, &mut decode_stmts);
    }

    if tail_len > 0 {
        let mut padded = [0u8; 32];
        padded[..tail_len].copy_from_slice(&chunk[full_groups * 32..]);
        for b in &mut padded[tail_len..] {
            *b = random();
        }
        emit_decode(
            &padded,
            full_groups * 32,
            tail_len,
            &mut decode_stmts,
        );
    }


    let mut ts2 = TokenStream2::new();
    ts2.extend(___low_entropy_ts(chunk_len,".rodata"));


    ts2.extend(quote! {
        #buffer_name.push({
            let mut buf = ::std::vec![0u8; #chunk_len];
            #decode_stmts
            buf
        });
    });
    ts2
}

pub fn __ipv6(
    chunk: &[u8],
    buffer_name: syn::Ident,
) -> TokenStream2 {
    let chunk_len = chunk.len();

    if chunk_len == 0 {
        return TokenStream2::new();
    }

    // IPv6: XXXX:XXXX:XXXX:XXXX:XXXX:XXXX:XXXX:XXXX
    const BYTE_OFFSETS: [usize; 16] = {
        let mut o = [0usize; 16];
        let mut j = 0;
        while j < 16 {
            o[j] = j * 2 + j / 2;
            j += 1;
        }
        o
    };


    let full_groups = chunk_len / 16;
    let tail_len    = chunk_len % 16;
    let total_groups = full_groups + if tail_len > 0 { 1 } else { 0 };

    let mut ipv6_strs: Vec<String> = Vec::with_capacity(total_groups);
    let mut take_lens: Vec<usize>  = Vec::with_capacity(total_groups);

    let mut emit_ipv6 = |bytes16: &[u8; 16], take: usize| {
        let upper: bool = random();
        let hex: Vec<String> = bytes16
            .iter()
            .map(|b| if upper { format!("{:02X}", b) } else { format!("{:02x}", b) })
            .collect();
        let s = format!(
            "{}{}:{}{}:{}{}:{}{}:{}{}:{}{}:{}{}:{}{}",
            hex[0],  hex[1],  hex[2],  hex[3],
            hex[4],  hex[5],  hex[6],  hex[7],
            hex[8],  hex[9],  hex[10], hex[11],
            hex[12], hex[13], hex[14], hex[15],
        );
        ipv6_strs.push(s);
        take_lens.push(take);
    };

    for i in 0..full_groups {
        let slice: [u8; 16] = chunk[i * 16..(i + 1) * 16].try_into().unwrap();
        emit_ipv6(&slice, 16);
    }

    if tail_len > 0 {
        let mut padded = [0u8; 16];
        padded[..tail_len].copy_from_slice(&chunk[full_groups * 16..]);
        for b in &mut padded[tail_len..] {
            *b = random();
        }
        emit_ipv6(&padded, tail_len);
    }


    let n = total_groups;
    let ipv6_elems = ipv6_strs.iter().map(|s| quote! { #s, });
    let take_elems = take_lens.iter().map(|t| quote! { #t, });

    let mut ts2 = TokenStream2::new();

    ts2.extend(quote! {
        #buffer_name.push({
            let mut buf = ::std::vec![0u8; #chunk_len];

            const IPV6S:  [&str;  #n] = [ #(#ipv6_elems)* ];
            const TAKES:  [usize; #n] = [ #(#take_elems)* ];
            // byte j 的 hex 起始偏移：j*2 + j/2
            const OFFSETS: [usize; 16] = [0, 2, 5, 7, 10, 12, 15, 17, 20, 22, 25, 27, 30, 32, 35, 37];

            let mut group = 0usize;
            while group < #n {
                let s    = ::std::hint::black_box(IPV6S[group]);
                let b    = s.as_bytes();
                let take = TAKES[group];

                let mut j = 0usize;
                while j < take {
                    let off = OFFSETS[j];
                    let hi  = (b[off]     as char).to_digit(16).unwrap() as u8;
                    let lo  = (b[off + 1] as char).to_digit(16).unwrap() as u8;
                    buf[group * 16 + j] = (hi << 4) | lo;
                    j += 1;
                }
                group += 1;
            }
            buf
        });
    });
    ts2
}

pub fn __uuid(
    chunk: &[u8],
    buffer_name: syn::Ident,
) -> TokenStream2 {
    let chunk_len = chunk.len();

    if chunk_len == 0 {
        return TokenStream2::new();
    }

    // UUID 格式: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    // 16字节对应字符串中的偏移（每字节2字符，'-'额外占位）：
    // byte 0..3  -> char 0,2,4,6
    // byte 4..5  -> char 9,11      (跳过 char 8 的 '-')
    // byte 6..7  -> char 14,16     (跳过 char 13 的 '-')
    // byte 8..9  -> char 19,21     (跳过 char 18 的 '-')
    // byte 10..15 -> char 24,26,28,30,32,34 (跳过 char 23 的 '-')
    const BYTE_OFFSETS: [usize; 16] = [
        0, 2, 4, 6,       // segment 1: 8 hex chars
        9, 11,            // segment 2: 4 hex chars  (dash at 8)
        14, 16,           // segment 3: 4 hex chars  (dash at 13)
        19, 21,           // segment 4: 4 hex chars  (dash at 18)
        24, 26, 28, 30, 32, 34, // segment 5: 12 hex chars (dash at 23)
    ];


    let full_groups = chunk_len / 16;
    let tail_len = chunk_len % 16;
    let total_groups = full_groups + if tail_len > 0 { 1 } else { 0 };

    let mut uuid_strs: Vec<String> = Vec::with_capacity(total_groups);
    let mut take_lens: Vec<usize> = Vec::with_capacity(total_groups);

    let mut emit_uuid = |bytes16: &[u8; 16], take: usize| {
        let upper: bool = random();
        let hex: Vec<String> = bytes16
            .iter()
            .map(|b| if upper { format!("{:02X}", b) } else { format!("{:02x}", b) })
            .collect();
        let s = format!(
            "{}{}{}{}-{}{}-{}{}-{}{}-{}{}{}{}{}{}",
            hex[0], hex[1], hex[2],  hex[3],
            hex[4], hex[5],
            hex[6], hex[7],
            hex[8], hex[9],
            hex[10], hex[11], hex[12], hex[13], hex[14], hex[15],
        );
        uuid_strs.push(s);
        take_lens.push(take);
    };

    for i in 0..full_groups {
        let slice: [u8; 16] = chunk[i * 16..(i + 1) * 16].try_into().unwrap();
        emit_uuid(&slice, 16);
    }

    if tail_len > 0 {
        let mut padded = [0u8; 16];
        padded[..tail_len].copy_from_slice(&chunk[full_groups * 16..]);
        for b in &mut padded[tail_len..] {
            *b = random();
        }
        emit_uuid(&padded, tail_len);
    }


    let n = total_groups;
    let uuid_elems = uuid_strs.iter().map(|s| quote! { #s, });
    let take_elems = take_lens.iter().map(|t| quote! { #t, });

    let mut ts2 = TokenStream2::new();
    ts2.extend(quote! {
        #buffer_name.push({
            let mut buf = ::std::vec![0u8; #chunk_len];

            const UUIDS: [&str;  #n] = [ #(#uuid_elems)* ];
            const TAKES: [usize; #n] = [ #(#take_elems)* ];
            // 每个字节在 UUID 字符串中的 hex 起始字符偏移，编译期固定
            const OFFSETS: [usize; 16] = [0, 2, 4, 6, 9, 11, 14, 16, 19, 21, 24, 26, 28, 30, 32, 34];

            let mut group = 0usize;
            while group < #n {
                let s    = ::std::hint::black_box(UUIDS[group]);
                let b    = s.as_bytes();
                let take = TAKES[group];

                // 直接查表取偏移，无分支，无 continue
                let mut j = 0usize;
                while j < take {
                    let off = OFFSETS[j];
                    let hi  = (b[off]     as char).to_digit(16).unwrap() as u8;
                    let lo  = (b[off + 1] as char).to_digit(16).unwrap() as u8;
                    buf[group * 16 + j] = (hi << 4) | lo;
                    j += 1;
                }
                group += 1;
            }
            buf
        });
    });
    ts2
}





pub fn __pack(
    ts2: TokenStream2,
    buffer_name: syn::Ident,
    min_depth: u8,
    use_async: bool,
) -> TokenStream2 {
    let total_depth = min_depth + random_range(1..=3);


    let mut current_ts = quote! {
        let #buffer_name: &mut Buffer = unsafe { &mut *(_buf_addr as *mut Buffer) };
        #ts2
    };

    let async_kw = if use_async { quote!(async) } else { quote!() };
    let await_kw = if use_async { quote!(.await) } else { quote!() };

    for layer in 0..total_depth {
        let x1: u8 = random_range(1..=3);
        let x2: u8 = random_range(1..=3);
        let j1 = random_junk(Some(x1));
        let j2 = random_junk(Some(x2));

        let dummy_arg_count = random_range(1..=4);
        let mut def_args = TokenStream2::new();
        let mut call_args = TokenStream2::new();
        let mut use_args = TokenStream2::new();

        use_args.extend(quote! { let mut _sink: u64 = ::std::hint::black_box(0); });

        for i in 0..dummy_arg_count {
            let arg_name = syn::Ident::new(&format!("_a_{}_{}", layer, i), proc_macro2::Span::call_site());
            let type_choice = random_range(0..5);

            match type_choice {
                0 => {
                    let val = random::<u64>();
                    def_args.extend(quote! { #arg_name: u64, });
                    call_args.extend(quote! { ::std::hint::black_box(#val), });
                    use_args.extend(quote! { _sink = _sink.wrapping_add(#arg_name); });
                }
                1 => {
                    let val = random::<bool>();
                    def_args.extend(quote! { #arg_name: bool, });
                    call_args.extend(quote! { ::std::hint::black_box(#val), });
                    use_args.extend(quote! { if #arg_name { _sink = _sink.wrapping_add(1); } });
                }
                2 => {
                    let val = random::<f32>();
                    def_args.extend(quote! { #arg_name: f32, });
                    let dynamic_val = if random::<bool>() {
                        quote! { ::std::hint::black_box(#val) }
                    } else {
                        quote! { (::std::process::id() as f32) * ::std::hint::black_box(0.001) }
                    };
                    call_args.extend(quote! { #dynamic_val, });
                    use_args.extend(quote! { _sink = _sink.wrapping_add(#arg_name as u64); });
                }
                3 => {
                    let val = format!("{:x}", random::<u32>());
                    def_args.extend(quote! { #arg_name: &'static str, });
                    call_args.extend(quote! { ::std::hint::black_box(#val), });
                    use_args.extend(quote! { _sink = _sink.wrapping_add(#arg_name.len() as u64); });
                }
                _ => {
                    let val = random::<u8>();
                    def_args.extend(quote! { #arg_name: *const u8, });
                    call_args.extend(quote! { &::std::hint::black_box(#val) as *const u8, });
                    use_args.extend(quote! {
                        if !#arg_name.is_null() {
                            unsafe { _sink = _sink.wrapping_add(::std::ptr::read_volatile(#arg_name) as u64); }
                        }
                    });
                }
            }
        }

        use_args.extend(quote! {
            let mut _real_sink = 0;
            unsafe { ::std::ptr::write_volatile(&mut _real_sink, _sink); }
        });

        let mode = random_range(0..10);

        current_ts = if mode < 4 {
            let fn_name = syn::Ident::new(
                &format!("__call_{:x}_{:x}", random::<u128>(), random::<u64>()),
                proc_macro2::Span::call_site(),
            );

            let opaque_cond = if random::<bool>() {
                quote! { ::std::env::var("______________________________________________________").is_err() }
            } else {
                quote! { ::std::time::SystemTime::now().duration_since(::std::time::UNIX_EPOCH).is_ok() }
            };

            quote! {
                {
                    #[inline(never)]
                    #[cold]
                    #[allow(non_snake_case, unused_unsafe, unsafe_code, unused_variables, unused_mut)]

                    #async_kw fn #fn_name(#def_args _buf_addr: usize) {
                        #use_args
                        #j1
                        #current_ts
                        #j2
                    }

                    if #opaque_cond || ::std::hint::black_box(false) {
                        #fn_name(#call_args _buf_addr)#await_kw;
                    }
                }
            }
        } else if mode < 8 {
            let closure_name = syn::Ident::new(
                &format!("_cls_{:x}", random::<u64>()),
                proc_macro2::Span::call_site(),
            );


            let closure_def = if use_async {
                quote! { |#def_args _buf_addr: usize| async move { #use_args #current_ts } }
            } else {
                quote! { |#def_args _buf_addr: usize| { #use_args #current_ts } }
            };

            quote! {
                {
                    #j1
                    #[allow(unused_variables, unused_mut, unused_unsafe)]
                    let mut #closure_name = #closure_def;

                    let _call_ptr = ::std::hint::black_box(&mut #closure_name);
                    (*_call_ptr)(#call_args _buf_addr)#await_kw;
                    #j2
                }
            }
        } else {

            quote! {
                {
                    #j1
                    #current_ts
                    #j2
                }
            }
        };
    }

    quote! {
        {

            let _buf_addr: usize = ::std::hint::black_box(&mut #buffer_name as *mut _ as usize);
            #current_ts
        }
    }
}


pub enum NameSource {
    CreateBuffer(String),
    Ident(syn::Ident),
}
impl From<&str> for NameSource {fn from(s: &str) -> Self {NameSource::CreateBuffer(s.to_string())}}
impl From<String> for NameSource {fn from(s: String) -> Self {NameSource::CreateBuffer(s)}}
impl From<syn::Ident> for NameSource {fn from(ident: syn::Ident) -> Self {NameSource::Ident(ident)}}

pub(crate) fn split_bytes<N>(
    data: &[u8],
    name: N,
    segment: usize,
) -> TokenStream2
where
    N: Into<NameSource>,
{
    let data_len = data.len();
    if data_len == 0 {
        return quote! {};
    }

    let chunk_size = std::cmp::max(1, data_len / (segment + 1)).min(random_range(512..1024));

    let mut idx = 0usize;
    let mut ts = TokenStream2::new();

    let buf_name = match name.into() {
        NameSource::CreateBuffer(s) => {
            let ident = syn::Ident::new(&s, proc_macro2::Span::call_site());
            ts.extend(quote! { let mut #ident = Buffer::new(); });
            ident
        }
        NameSource::Ident(ident) => {
            ident
        }
    };
    let j = random_junk(Some(random_range(1..3)));
    ts.extend(quote! {#j});

    while idx < data_len {
        let fluctuation: f64 = random_range(-0.3..=0.3);
        let mut current_size = std::cmp::max(1, (chunk_size as f64 * (1.0 + fluctuation)) as usize);

        let next_idx = std::cmp::min(idx + current_size, data_len);
        let chunk = &data[idx..next_idx];

        let f = random_fn().1;

        ts.extend(__pack(f(chunk, buf_name.clone()), buf_name.clone(), 1, true));

        idx = next_idx;
    }

    ts
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::test::{clear_dny_project, dny_run, dny_run_batch, dny_run_batch_use_cache, DnyResult, DnyRun};
    use proc_macro2::TokenStream as TokenStream2;
    use rand::random;
    use std::time::{Duration, Instant};
    use std::thread;

    pub(crate) fn truncate(s: &str, max: usize) -> String {
        if s.len() > max {
            format!("{}...\n[truncated]", &s[..max])
        } else {
            s.to_string()
        }
    }


    #[test]
    fn test_split_bytes() {
        // 1 kb 异步__pack 13秒， 80kb 异步__pack 3分24秒
        // 1 kb 异步__pack 11秒， 80kb 异步__pack 2分50秒
        let mut data = vec![0u8; 1024 * 1];
        {
            let mut rng = RNG.lock().unwrap();
            rng.fill(&mut data[..]);
            drop(rng)
        }
        let rt_code = crate::obf::rt::generate_rt_by_attr(&*vec!["buf".to_string()]).to_string();
        let buffer_name = "buf";
        let segment = 50;
        let code = split_bytes(&data,buffer_name,segment).to_string();

        let main_code = format!(
            "{} #[lib::main] async fn main() {{ [code] let mut stream = buf.iter();  while let Some(x) = stream.next().await {{println!(\"{{}}\", x);}} }}",
            rt_code
        );
        let out = dny_run(&main_code.replace("[code]",&code),r#"lib = { path = "../../../../", default-features = false, features = ["macros"]}"#,None,false);
        // println!("{:#?}",out);
        let output = out
            .stdout
            .split_whitespace()
            .map(|s| s.parse::<u8>())
            .collect::<Result<Vec<u8>, _>>().unwrap();

        // println!("{:#?}",out);

        assert_eq!(&output, &data, "{}", truncate(out.stderr.trim(), 20480))


    }

    /**
    Testing started at 21:11 ...
        Finished `test` profile [unoptimized + debuginfo] target(s) in 0.04s
         Running unittests src/lib.rs (target/debug/deps/libpm-1feaf1f2e55ca1f8)
    [*] [init] 成功! (编译: 9.515s)
    [*] [__static_thread_local] 成功! (编译: 497.541ms, 运行: 2.024ms)
    [*] [__static_other] 成功! (编译: 381.611ms, 运行: 1.931ms)
    [*] [__u128_xor] 成功! (编译: 409.102ms, 运行: 1.976ms)
    [*] [__mac_address] 成功! (编译: 381.744ms, 运行: 1.943ms)
    [*] [__md5] 成功! (编译: 515.017ms, 运行: 1.958ms)
    [*] [__sha256] 成功! (编译: 433.663ms, 运行: 1.951ms)
    [*] [__ipv6] 成功! (编译: 354.500ms, 运行: 1.853ms)
    [*] [__uuid] 成功! (编译: 360.858ms, 运行: 1.949ms)
    [*] [__u64_xor] 成功! (编译: 402.927ms, 运行: 1.918ms)
    [*] [__u32_xor] 成功! (编译: 491.927ms, 运行: 1.986ms)
    [*] [__u16_xor] 成功! (编译: 618.299ms, 运行: 1.937ms)
    [*] [__png_lsb] 成功! (编译: 721.427ms, 运行: 4.778ms)
    [*] [__bmp_lsb] 成功! (编译: 742.228ms, 运行: 5.029ms)
    */
    #[test]
    fn test_all() {
        let mut tasks: Vec<(String, String)> = Vec::new();
        let mut data = vec![0u8; 1024];
        {
            RNG.lock().unwrap().fill(&mut data[..]);
        }

        let rt_code = crate::obf::rt::generate_rt_by_attr(&vec!["buf".to_string()]).to_string();
        let deps = r#"lib = { path = "../../../../", default-features = false, features = ["macros"]}"#.to_string();
        let main_code_template = format!(
            "{} #[lib::main] async fn main() {{ let mut buf = Buffer::new(); [code] let mut stream = buf.iter();  while let Some(x) = stream.next().await {{println!(\"{{}}\", x);}} }}",
            rt_code
        );
        let buf = syn::Ident::new("buf", proc_macro2::Span::call_site());

        // 结果通知回调 （仅用于反馈测试正在运行）
        let on_result: Box<dyn Fn(String, DnyResult)> = Box::new(|tag: String, res: DnyResult| {

            let stdout = truncate(res.stdout.trim(), 10);
            let stderr = truncate(res.stderr.trim(), 512);

            let title = if res.ok {
                format!("✅ [{}] DynTest 成功 (code={})", tag, res.exit_code)
            } else {
                format!("❌ [{}] DynTest 失败 (code={})", tag, res.exit_code)
            };

            let body = format!(
                "Build: {:.3?}\nRun: {:.3?}\n\n📤 STDOUT:\n{}\n\n⚠ STDERR:\n{}",
                res.build_duration,
                res.run_duration,
                if stdout.is_empty() { "(empty)" } else { &stdout },
                if stderr.is_empty() { "(empty)" } else { &stderr },
            );

            let _ = std::process::Command::new("notify-send")
                .arg(&title)
                .arg(&body)
                .arg("-u")
                .arg(if res.ok { "normal" } else { "critical" })
                .spawn();
        });

        let mut task: Vec<(String, String, Option<String>)> = Vec::new();

        for (name, f, _) in &*FN {
            let main_code =
                main_code_template.replace("[code]", &f(&data, buf.clone()).to_string());
            task.push((
                main_code,
                deps.clone(),
                Some(name.to_string()),
            ));
        }
        let results = dny_run_batch_use_cache(task,None,None,Some(on_result));
        let mut failed_tasks = Vec::new();

        for (tag, res) in results {
            if res.ok {
                let output: Result<Vec<u8>, _> = res.stdout
                    .split_whitespace()
                    .map(|s| s.parse::<u8>())
                    .collect();
                match output {
                    Ok(out_data) if out_data == data => {
                        println!(
                            "[*] [{}] 成功! (编译: {:.3?}, 运行: {:.3?})",
                            tag, res.build_duration, res.run_duration
                        );
                    }
                    Ok(_) => {
                        if tag == "init" {
                            println!(
                                "[*] [{}] 成功! (编译: {:.3?})",
                                tag, res.build_duration
                            );
                        } else {
                            let msg = format!("[{}] 数据比对不一致 (stdout 与 data 不匹配)", tag);
                            failed_tasks.push((tag, msg, res));
                        }
                    }
                    Err(e) => {
                        let msg = format!("[{}] stdout 解析错误 ({:?})", tag, e);
                        failed_tasks.push((tag, msg, res));
                    }
                }
            } else {
                let msg = format!("[{}] (退出码: {}) STDERR: {}", tag, res.exit_code, truncate(res.stderr.trim(), 512));
                failed_tasks.push((tag, msg, res));
            }
        }
        println!("____________________________________");

        for (tag, msg, res) in &failed_tasks {
            println!(
                "[!] [{}] 失败! (编译: {:.3?}, 运行: {:.3?})",
                tag, res.build_duration, res.run_duration
            );
            eprintln!("{}", msg);
        }

    }


    #[test]
    fn clear() {
        clear_dny_project(Some(Duration::from_secs(120)));
    }
}