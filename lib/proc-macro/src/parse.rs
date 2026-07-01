#![allow(unused)]
use syn::parse::{discouraged::Speculative, Parse, ParseStream};
use syn::Expr;
use std::{env, path::PathBuf};

pub(crate) struct BytesInput {
    pub(crate) bytes: Vec<u8>
}

impl Parse for BytesInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // 1. 尝试解析为字节字符串: b"..."
        let fork = input.fork();
        if let Ok(lit) = fork.parse::<syn::LitByteStr>() {
            input.advance_to(&fork);
            return Ok(BytesInput {
                bytes: lit.value(),
            });
        }

        // 2. 尝试解析为常规字符串: "..." (将其作为相对于 Cargo.toml 的文件路径)
        let fork = input.fork();
        if let Ok(lit_str) = fork.parse::<syn::LitStr>() {
            input.advance_to(&fork);

            // 获取调用该宏的 crate 的 Cargo.toml 所在目录
            let manifest_dir = env::var("CARGO_MANIFEST_DIR").map_err(|_| {
                syn::Error::new(lit_str.span(), "无法获取 CARGO_MANIFEST_DIR 环境变量")
            })?;

            // 拼接出绝对路径
            let mut path = PathBuf::from(manifest_dir);
            path.push(lit_str.value());

            // 在编译期读取文件内容
            let bytes = std::fs::read(&path).map_err(|e| {
                syn::Error::new(
                    lit_str.span(),
                    format!("无法读取文件 '{}': {}", path.display(), e),
                )
            })?;

            return Ok(BytesInput {
                bytes,
            });
        }

        // 3. 尝试解析为数组: [1, 2, 3]
        let arr: syn::ExprArray = input.parse().map_err(|e| {
            syn::Error::new(e.span(), "obfbytes! 需要一个字节字符串 (b\"...\"), 文件路径 (\"...\"), 或数组 ([1, 2])")
        })?;

        let mut bytes = Vec::new();
        for expr in arr.elems {
            match expr {
                Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(lit_int), .. }) => {
                    let b = lit_int.base10_parse::<u8>().map_err(|_| {
                        syn::Error::new_spanned(&lit_int, "值必须是有效的 u8 (0-255)")
                    })?;
                    bytes.push(b);
                }
                _ => return Err(syn::Error::new_spanned(expr, "obfbytes! 数组中只允许整数")),
            }
        }

        Ok(BytesInput { bytes })
    }
}