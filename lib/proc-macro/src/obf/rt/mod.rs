//lib/proc-macro/src/obf/rt/mod.rs
mod str;
mod bytes;
mod buf;
mod s1;
mod zstd;
pub(crate) mod image;
pub(crate) mod base;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// 运行时核心构建函数
pub fn build(attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
    // 提取参数列表
    let args: Vec<String> = attr
        .into_iter()
        .map(|token| token.to_string())
        .filter(|s| s != ",")
        .collect();

    let rt_tokens = generate_rt_by_attr(&args);

    quote! {
        #rt_tokens
        #item
    }
}

/// 根据传入的属性条件选择性生成运行时
pub fn generate_rt_by_attr(args: &[String]) -> TokenStream2 {
    let mut rt_tokens = TokenStream2::new();
    let is_empty = args.is_empty();

    let has = |aliases: &[&str]| -> bool {
        args.iter().any(|arg| aliases.contains(&arg.as_str()))
    };

    rt_tokens.extend(emit_file(include_str!("base.rs")));


    #[cfg(feature = "s1")]
    {
        if is_empty || has(&["s1"]) {
            rt_tokens.extend(emit_file(include_str!("s1.rs")));
        }
    }


    #[cfg(any(feature = "s1", feature = "s2"))]
    {
        if is_empty || has(&["str", "s", "s1"]) { // 自定义堆栈字符串
            rt_tokens.extend(emit_file(include_str!("./str.rs")));
        }
    }

    // if is_empty || has(&["bytes", "b"]) {
    //     rt_tokens.extend(emit_file(include_str!("./bytes.rs")));
    // }

    #[cfg(feature = "buf")]
    {
        if is_empty || has(&["buf", "b"]) {
            let src = include_str!("./image.rs").replace("image::", "::lib::image::");
            rt_tokens.extend(emit_file(&src));
            let src = include_str!("./buf.rs").replace("futures_core::", "::lib::__futures_core::")
                .replace("tokio_stream::", "::lib::__tokio_stream::")
                .replace("async_stream::", "::lib::__async_stream::")
                .replace("tokio::", "::lib::__tokio::");
            rt_tokens.extend(emit_file(&src));
        }
    }




    rt_tokens
}

fn emit_file(source: &str) -> TokenStream2 {
    // 确保代码结构符合要求即可
    let clean_source = source.split("#[cfg(test)]").next().unwrap_or(source);
    let file = syn::parse_file(clean_source).expect("failed to parse rt source file");
    let items = file.items;
    quote! { #(#items)* }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        include::clean_include!("src/obf/rt/base.rs");
        include::clean_include!("src/obf/rt/str.rs");

        let _x = HeapStr::default();
    }
}