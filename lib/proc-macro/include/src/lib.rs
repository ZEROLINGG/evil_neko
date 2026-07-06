use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{bracketed, parenthesized, parse_macro_input, LitStr, Token};
use std::env;
use std::fs;
use std::path::PathBuf;

/// 用于解析括号内的单个元组： ("old", "new")
struct ReplaceTuple {
    old: LitStr,
    new: LitStr,
}

impl Parse for ReplaceTuple {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        parenthesized!(content in input); // 匹配 ( ... )
        let old: LitStr = content.parse()?;
        content.parse::<Token![,]>()?;    // 匹配逗号
        let new: LitStr = content.parse()?;
        Ok(ReplaceTuple { old, new })
    }
}

/// 用于解析整个宏的输入参数
struct CleanIncludeInput {
    path: LitStr,
    replacements: Vec<(String, String)>,
}

impl Parse for CleanIncludeInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // 1. 解析路径字符串
        let path: LitStr = input.parse()?;
        let mut replacements = Vec::new();

        // 2. 如果路径后有逗号，检查是否有替换规则数组
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?; // 消耗掉逗号

            // 检查下一个 token 是否是中括号 `[`
            if input.peek(syn::token::Bracket) {
                let content;
                bracketed!(content in input); // 匹配 [ ... ]

                // 解析中括号内的元组，允许末尾带有逗号
                let tuples = Punctuated::<ReplaceTuple, Token![,]>::parse_terminated(&content)?;

                for tuple in tuples {
                    replacements.push((tuple.old.value(), tuple.new.value()));
                }

                // 允许数组外部也有尾随逗号 (例如: clean_include!("..", [...],); )
                if input.peek(Token![,]) {
                    input.parse::<Token![,]>()?;
                }
            }
        }

        Ok(CleanIncludeInput { path, replacements })
    }
}

#[proc_macro]
pub fn clean_include(input: TokenStream) -> TokenStream {
    // 使用自定义的解析逻辑解析输入
    let parsed_input = parse_macro_input!(input as CleanIncludeInput);
    let rel_path = parsed_input.path.value();

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let file_path = PathBuf::from(manifest_dir).join(&rel_path);
    let abs_path_str = file_path.to_str().expect("Path is not valid UTF-8");

    let source = fs::read_to_string(&file_path)
        .unwrap_or_else(|_| panic!("Failed to read clean_include file: {:?}", file_path));

    // 截取测试模块之前的代码
    let mut clean_source = source.split("#[cfg(test)]").next().unwrap_or(&source).to_string();

    // =============== 新增逻辑：执行字符串替换 ===============
    for (old, new) in parsed_input.replacements {
        clean_source = clean_source.replace(&old, &new);
    }
    // =======================================================

    // 将替换后的字符串解析为 Rust AST 树
    let file = syn::parse_file(&clean_source)
        .unwrap_or_else(|e| panic!("failed to parse rt source file (after replacements): {}", e));

    let items = file.items;

    let expanded = quote! {
        const _: &[u8] = include_bytes!(#abs_path_str);
        #(#items)* 
    };

    TokenStream::from(expanded)
}