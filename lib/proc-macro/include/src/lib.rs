use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, LitStr};
use std::env;
use std::fs;
use std::path::PathBuf;

#[proc_macro]
pub fn clean_include(input: TokenStream) -> TokenStream {
    let path_lit = parse_macro_input!(input as LitStr);
    let rel_path = path_lit.value();

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let file_path = PathBuf::from(manifest_dir)
        .join(&rel_path);
    let abs_path_str = file_path.to_str().expect("Path is not valid UTF-8");


    let source = fs::read_to_string(&file_path)
        .unwrap_or_else(|_| panic!("Failed to read clean_include file: {:?}", file_path));


    let clean_source = source.split("#[cfg(test)]").next().unwrap_or(&source);

    let file = syn::parse_file(clean_source).expect("failed to parse rt source file");

    let items = file.items;

    let expanded = quote! {
        const _: &[u8] = include_bytes!(#abs_path_str);
        #(#items)* 
    };

    TokenStream::from(expanded)
}