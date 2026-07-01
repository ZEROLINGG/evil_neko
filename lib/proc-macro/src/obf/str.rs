//lib/proc-macro/src/obf/str.rs
#![allow(unused)]
use proc_macro2::{Literal as Literal2, TokenStream as TokenStream2};
use quote::quote;
use rand::RngExt;
use syn::parse::{Parse, ParseStream, Parser};
use syn::{punctuated::Punctuated, Expr, LitStr, Token};

macro_rules! parse {
    ($input:expr) => {
        parse!($input, 512)
    };

    ($input:expr, $limit:expr) => {{
        let lit = match syn::parse2::<syn::LitStr>($input) {
            Ok(lit) => lit,
            Err(err) => return err.to_compile_error(),
        };

        let string = lit.value();
        let limit: usize = $limit;

        if string.len() > limit {
            let msg = format!("过长的字符串，最大允许 {} 字节", limit);
            return syn::Error::new(lit.span(), msg)
                .to_compile_error();
        }

        string
    }};
}
#[cfg(feature = "s1")]
fn obf_1(input: String) -> TokenStream2 {
    let data = input.as_bytes();
    let data_len = data.len();

    if data_len == 0 {
        return quote! { StackStr::<0>::from([]) };
    }

    let mut rng: rand::rngs::SmallRng = rand::make_rng();

    let key: u128 = rng.random();
    let key_tok = Literal2::u128_unsuffixed(key);
    let data_len_tok = Literal2::usize_unsuffixed(data_len);

    let mut ts2 = TokenStream2::new();

    let mut stream_key = key;

    let mut idx = 0;
    while idx < data_len {
        let remain = data_len - idx;

        let current_size = if remain >= 256 && rng.random_bool(0.4) {
            rng.random_range(64..128)
        } else if remain >= 128 && rng.random_bool(0.5) {
            rng.random_range(32..64)
        } else if remain >= 64 && rng.random_bool(0.6) {
            rng.random_range(16..32)
        } else if remain >= 32 && rng.random_bool(0.3) {
            rng.random_range(9..16)
        } else if remain >= 16 && rng.random_bool(0.75) {
            16
        } else if remain >= 8 && rng.random_bool(0.75) {
            8
        } else if remain >= 4 && rng.random_bool(0.75) {
            4
        } else if remain >= 2 && rng.random_bool(0.75) {
            2
        } else {
            1
        };

        let next_idx = idx + current_size;
        let chunk = &data[idx..next_idx];
        let idx_start = idx;
        let idx_end = next_idx;

        let use_le = rng.random::<bool>();
        let method = if use_le { quote!(to_le_bytes) } else { quote!(to_be_bytes) };

        macro_rules! uint_branch {
            ($size:expr, $ty:ident, $lit_fn:ident) => {{
                stream_key = crate::obf::rt::base::next_u128(stream_key);
                let mask = stream_key as $ty;

                let mut arr = [0u8; $size];
                arr.copy_from_slice(chunk);

                let orig_data = if use_le { $ty::from_le_bytes(arr) } else { $ty::from_be_bytes(arr) };
                let obf_tok = Literal2::$lit_fn(orig_data ^ mask);

                ts2.extend(quote! {
                    rt_key = next_u128(rt_key);
                    buffer[#idx_start..#idx_end].copy_from_slice(
                        &(#obf_tok ^ (rt_key as $ty)).#method()
                    );
                });
            }};
        }

        match current_size {
            16 => uint_branch!(16, u128, u128_unsuffixed),
            8  => uint_branch!( 8,  u64,  u64_unsuffixed),
            4  => uint_branch!( 4,  u32,  u32_unsuffixed),
            2  => uint_branch!( 2,  u16,  u16_unsuffixed),

            1 => {
                stream_key = crate::obf::rt::base::next_u128(stream_key);
                let obf_tok = Literal2::u8_unsuffixed(chunk[0] ^ stream_key as u8);
                ts2.extend(quote! {
                    rt_key = next_u128(rt_key);
                    buffer[#idx_start] = #obf_tok ^ (rt_key as u8);
                });
            }
            _ => {

                let encrypt_table = [2, 4, 1, 6, 7, 3, 0, 5];

                let obf_bytes: Vec<u8> = chunk
                    .iter()
                    .enumerate()
                    .map(|(i, &p)| {
                        stream_key = crate::obf::rt::base::next_u128(stream_key);
                        let t_inv = crate::obf::rt::base::__swap_u8_bits(p, encrypt_table);
                        t_inv ^ stream_key as u8 ^ i as u8
                    })
                    .collect();

                let obf_array_tok = proc_macro2::Literal::byte_string(&obf_bytes);
                let idx_start_tok = Literal2::usize_unsuffixed(idx_start);
                let current_size_tok = Literal2::usize_unsuffixed(current_size);

                ts2.extend(quote! {
                    {
                        let encrypted: &[u8; #current_size_tok] = #obf_array_tok;
                        unsafe {
                            rt_key = __obf_1_decrypt_bytes(
                                buffer_ptr.byte_add(1),
                                encrypted.as_ptr(),
                                #idx_start_tok,
                                #current_size_tok,
                                rt_key,
                            );
                        }
                    }
                });
            }
        }

        idx = next_idx;
    }

    quote! {
        {
            let mut buffer = [0u8; #data_len_tok];
            let key: u128 = ::std::hint::black_box(#key_tok);
            let buffer_ptr = ::std::hint::black_box(__ptr_calc(buffer.as_mut_ptr(), #data_len_tok, key as usize));
            let mut x = || {
                let mut rt_key = key;
                #ts2
            };
            ::std::hint::black_box(x());
            unsafe { StackStr::<#data_len_tok>::from((buffer_ptr.byte_add(1),#data_len_tok)) }
        }
    }
}

#[cfg(feature = "s2")]
fn obf_2(input: String) -> TokenStream2 {
    let data = input.as_bytes();
    let data_len = data.len();

    if data_len == 0 {
        return quote! {
            async {
                HeapStr::default()
            }.await
        };
    }

    let buffer = syn::Ident::new("__buffer", proc_macro2::Span::call_site());
    // 包含随机分片，异步混淆，控制流程图混淆，花指令混淆，熵控制，各种隐写……
    let ts = crate::obf::split_bytes::split_bytes(data, buffer.clone(), 30);
    quote! {
        async {
            let mut #buffer = Buffer::new();
            #ts
            HeapStr::new(#buffer.to_vec().await)
        }.await
    }
}

#[cfg(feature = "s1")]
pub fn s(input: TokenStream2) -> TokenStream2 {
    let string = parse!(input);
    obf_1(string)
}

/// 立即消费场景使用
#[cfg(feature = "s1")]
pub fn ss(input: TokenStream2) -> TokenStream2 {
    let string = parse!(input);
    let ts = obf_1(string);
    quote! { 
        { #ts }.as_str()
    }
}

/// 不安全的String接口
#[cfg(feature = "s1")]
pub fn sss(input: TokenStream2) -> TokenStream2 {
    let string = parse!(input);
    let ts = obf_1(string);
    quote! { 
        { 
            let ss = {#ts};
            let string = ss.to_string();
            string
        }
    }
}
#[cfg(feature = "s2")]
pub fn s2(input: TokenStream2) -> TokenStream2 {
    let string = parse!(input, 1024 * 1024 * 128);
    obf_2(string)
}

fn __add<F>(input: TokenStream2, obf_fn: F) -> TokenStream2
where
    F: Fn(String) -> TokenStream2,
{
    let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
    let args = match parser.parse2(input) {
        Ok(args) => args,
        Err(err) => return err.to_compile_error(),
    };

    let mut iter = args.into_iter();

    let first = match iter.next() {
        Some(arg) => arg,
        None => return quote! { HeapStr::default() },
    };

    let mut current = match first {
        Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit_str), .. }) => {
            let obf_toks = obf_fn(lit_str.value());
            quote! { (#obf_toks) }
        }
        other => {
            quote! { HeapStr::default().append_display(&(#other)) }
        }
    };

    for arg in iter {
        let next_expr = match arg {
            Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit_str), .. }) => {
                let obf_toks = obf_fn(lit_str.value());
                quote! { (#obf_toks) }
            }
            other => quote! { &(#other) },
        };

        current = quote! { (#current).append_display(#next_expr) };
    }

    current
}

#[cfg(feature = "s1")]
pub fn s_add(input: TokenStream2) -> TokenStream2 {
    __add(input, obf_1)
}
#[cfg(feature = "s2")]
pub fn s2_add(input: TokenStream2) -> TokenStream2 {
    __add(input, obf_2)
}

struct FormatArgs {
    fmt_lit: LitStr,
    rest: TokenStream2,
}

impl Parse for FormatArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let fmt_lit: LitStr = input.parse().map_err(|e| {
            syn::Error::new(e.span(), "obfstr_fmt! requires a string literal as the first argument")
        })?;

        let rest = if input.peek(Token![,]) {
            let _comma: Token![,] = input.parse()?;
            input.parse()?
        } else {
            TokenStream2::new()
        };

        Ok(FormatArgs { fmt_lit, rest })
    }
}

/// 拆分后的格式化字符串块
enum Chunk {
    Text(String),
    Placeholder(String),
}

/// 类似 format! 的使用体验，但对格式字符串中的字面量部分使用 obfstr 混淆。好用但安全性较低，一般推荐使用add宏
fn __fmt<F>(input: TokenStream2, obf_fn: F) -> TokenStream2
where
    F: Fn(String) -> TokenStream2,
{
    let args = match syn::parse2::<FormatArgs>(input) {
        Ok(args) => args,
        Err(err) => return err.to_compile_error(),
    };
    let fmt_str = args.fmt_lit.value();

    let chunks = match parse_format_string(&fmt_str) {
        Ok(c) => c,
        Err(err) => {
            return syn::Error::new(args.fmt_lit.span(), err).to_compile_error();
        }
    };

    let mut new_fmt = String::new();
    let mut obf_injections = Vec::new();
    let mut obf_idx = 0;

    for chunk in chunks {
        match chunk {
            Chunk::Text(text) => {
                let unescaped_text = text.replace("{{", "{").replace("}}", "}");
                let arg_name = quote::format_ident!("___fmt_chunk_{}__", obf_idx.to_string());
                new_fmt.push_str(&format!("{{{}}}", arg_name));

                // 调用传入的混淆函数
                let ts = obf_fn(unescaped_text);
                obf_injections.push(quote! { #arg_name = #ts });

                obf_idx += 1;
            }
            Chunk::Placeholder(ph) => {
                new_fmt.push_str(&ph);
            }
        }
    }

    let new_fmt_lit = LitStr::new(&new_fmt, args.fmt_lit.span());
    let mut final_args = args.rest;

    let has_trailing_comma = final_args.clone().into_iter().last().map_or(false, |tt| {
        matches!(tt, proc_macro2::TokenTree::Punct(p) if p.as_char() == ',')
    });

    if !final_args.is_empty() && !obf_injections.is_empty() && !has_trailing_comma {
        final_args.extend(quote! { , });
    }

    if !obf_injections.is_empty() {
        let inj_tokens = quote! { #(#obf_injections),* };
        final_args.extend(inj_tokens);
    }

    quote! {
        format!(#new_fmt_lit, #final_args)
    }
}
#[cfg(feature = "s1")]
pub fn s_fmt(input: TokenStream2) -> TokenStream2 {
    __fmt(input, obf_1)
}
#[cfg(feature = "s2")]
pub fn s2_fmt(input: TokenStream2) -> TokenStream2 {
    __fmt(input, obf_2)
}

/// 稳健地解析格式化字符串
fn parse_format_string(s: &str) -> Result<Vec<Chunk>, String> {
    let mut chunks = Vec::new();
    let mut chars = s.chars().peekable();
    let mut text = String::new();

    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
                text.push_str("{{");
            } else {
                if !text.is_empty() {
                    chunks.push(Chunk::Text(std::mem::take(&mut text)));
                }
                let mut ph = String::from("{");
                let mut closed = false;
                for pc in chars.by_ref() {
                    ph.push(pc);
                    if pc == '}' {
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    return Err("Unclosed '{' in format string".into());
                }
                chunks.push(Chunk::Placeholder(ph));
            }
        } else if c == '}' {
            if chars.peek() == Some(&'}') {
                chars.next();
                text.push_str("}}");
            } else {
                return Err("Unmatched '}' in format string".into());
            }
        } else {
            text.push(c);
        }
    }
    if !text.is_empty() {
        chunks.push(Chunk::Text(text));
    }
    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use super::*;
    use crate::test::{clear_dny_project, dny_run_batch_use_cache};

    fn random_strings(count: usize, length: usize) -> Vec<String> {
        let mut rng: rand::rngs::SmallRng = rand::make_rng();
        let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789お客様の位置情報にもとづき、その地域に適したコンテンツ、サービス、価格が確認できる日本のwebサイトをおすすめします。".chars().collect();
        (0..=count).map(|_| {
            (0..=length).map(|_| chars[rng.random_range(0..chars.len())]).collect()
        }).collect()
    }

    fn obf(input: String, use_obfstr: bool, use_obf1: bool) -> String {
        let code;
        if use_obfstr {
            code = format!("obfstr::obfstr!(\"{}\")", input)
        } else {
            code = if use_obf1 { obf_1(input.clone()).to_string() } else { obf_2(input.clone()).to_string() }
        }
        format!("println!(\"{{}}\",::std::hint::black_box({}));", code)
    }

    /**
    [count: 128 length: 128]
    _____________
    [init]
    exit_code: 0, build: 5.81s, run: 0.00ns
    error:

    _____________
    [obfstr_code]
    exit_code: 0, build: 2.08s, run: 1.52ms
    error:

    stdout ok!
    _____________
    [obf1_code]
    exit_code: 0, build: 2.29s, run: 3.39ms
    error:

    stdout ok!

    进程已结束，退出代码为 0*/
    #[test]
    fn test_s1() {
        let count = 128;
        let length = 128;
        let test_strings = random_strings(count, length);
        println!("[count: {} length: {}]", count, length );


        let expected_output = test_strings.join("\n");

        let deps = r#"
        lib = { path = "../../../../", default-features = false, features = ["macros-s1"] }
        obfstr = "0.4"
        "#.to_string();

        let rt_code = crate::obf::rt::generate_rt_by_attr(&vec!["s1".to_string()]).to_string();
        let main_code_template = format!("fn main() {{ {} [obfcode] }}", rt_code);

        let mut task: Vec<(String, String, Option<String>)> = Vec::new();

        let obf1_code = test_strings.iter().map(|s| {obf(s.clone(), false, true) }).collect::<Vec<_>>().join(" ");
        let obfstr_code = test_strings.iter().map(|s| {obf(s.clone(), true, true) }).collect::<Vec<_>>().join(" ");
        task.push((main_code_template.clone().replace("[obfcode]", &obfstr_code), deps.clone(), Some("obfstr_code".to_string())));
        task.push((main_code_template.clone().replace("[obfcode]", &obf1_code), deps.clone(), Some("obf1_code".to_string())));

        let results = dny_run_batch_use_cache(task, None, None, None);

        for result in results {
            println!("_____________");
            println!("[{}]", result.0);
            println!("exit_code: {}, build: {:.2?}, run: {:.2?}", result.1.exit_code, result.1.build_duration, result.1.run_duration);
            println!("error: \n{}", crate::obf::split_bytes::tests::truncate(&*result.1.stderr, 1024));

            if result.0 == "init" {
                continue;
            }

            let actual_stdout = result.1.stdout.replace("\r\n", "\n");
            let actual_trimmed = actual_stdout.trim();
            let expected_trimmed = expected_output.trim();

            if actual_trimmed == expected_trimmed {
                println!("stdout ok!");
            } else {
                println!("stdout error");
                assert_eq!(actual_trimmed, expected_trimmed);
            }
        }
    }
    /**
    [test_s2][count: 2 length: 16]
    _____________
    [init]
    exit_code: 0, build: 6.88s, run: 0.00ns
    error:

    _____________
    [obfstr_code]
    exit_code: 0, build: 467.70ms, run: 1.26ms
    error:

    stdout ok!
    _____________
    [obf2_code]
    exit_code: 0, build: 9.61s, run: 7.67ms
    error:

    stdout ok!*/
    #[test]
    fn test_s2() {
        let count = 8;
        let length = 512;
        let test_strings = random_strings(count, length);

        println!("[test_s2][count: {} length: {}]", count, length );


        let expected_output = test_strings.join("\n");

        let deps = r#"
        lib = { path = "../../../../", default-features = false, features = ["macros"] }
        obfstr = "0.4"
        "#.to_string();

        let rt_code = crate::obf::rt::generate_rt_by_attr(&vec!["s".to_string(),"b".to_string()]).to_string();
        let main_code_template = format!("#[lib::main] async fn main() {{ {} [obfcode] }}", rt_code);

        let mut task: Vec<(String, String, Option<String>)> = Vec::new();

        let obf2_code = test_strings.iter().map(|s| {obf(s.clone(), false, false) }).collect::<Vec<_>>().join(" ");
        let obfstr_code = test_strings.iter().map(|s| {obf(s.clone(), true, false) }).collect::<Vec<_>>().join(" ");
        task.push((main_code_template.clone().replace("[obfcode]", &obfstr_code), deps.clone(), Some("obfstr_code".to_string())));
        task.push((main_code_template.clone().replace("[obfcode]", &obf2_code), deps.clone(), Some("obf2_code".to_string())));

        let results = dny_run_batch_use_cache(task, None, None, None);

        for result in results {
            println!("_____________");
            println!("[{}]", result.0);
            println!("exit_code: {}, build: {:.2?}, run: {:.2?}", result.1.exit_code, result.1.build_duration, result.1.run_duration);
            println!("error: \n{}", crate::obf::split_bytes::tests::truncate(&*result.1.stderr, 1024));

            if result.0 == "init" {
                continue;
            }

            let actual_stdout = result.1.stdout.replace("\r\n", "\n");
            let actual_trimmed = actual_stdout.trim();
            let expected_trimmed = expected_output.trim();

            if actual_trimmed == expected_trimmed {
                println!("stdout ok!");
            } else {
                println!("stdout error");
                assert_eq!(actual_trimmed, expected_trimmed);
            }
        }
    }
    #[test]
    fn clear() {
        clear_dny_project(Some(Duration::from_secs(120)));
    }
}