

// lib/proc-macro/src/lib.rs
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Expr};

mod junk;
mod obf;
mod test;
mod parse;

#[proc_macro_attribute]
pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr2 = proc_macro2::TokenStream::from(attr);
    let item2 = proc_macro2::TokenStream::from(item);

    let args = if attr2.is_empty() {
        quote! { crate = "::lib::runtime::tokio" }
    } else {
        quote! { crate = "::lib::runtime::tokio", #attr2 }
    };

    let expanded = quote! {
        #[::lib::runtime::tokio::main(#args)]
        #item2
    };
    expanded.into()
}

/// 快速打印宏
#[proc_macro]
pub fn sprint(input: TokenStream) -> TokenStream {
    let expr = parse_macro_input!(input as Expr);
    quote! {
        {
            use std::io::Write;

            std::io::stdout()
                .write_all((#expr).as_bytes())
                .unwrap();

            std::io::stdout()
                .write_all(b"\n")
                .unwrap();
        }
    }
    .into()
}

#[proc_macro_attribute]
pub fn rt(attr: TokenStream, item: TokenStream) -> TokenStream {
    obf::rt::build(attr.into(), item.into()).into()
}

// ==========================================
// OBF 混淆模块包装入口
// ==========================================
#[cfg(feature = "buf")]
#[proc_macro]
pub fn buffer(input: TokenStream) -> TokenStream {
    obf::buf::buf(input.into()).into()
}
#[cfg(feature = "s1")]
#[proc_macro]
pub fn s(input: TokenStream) -> TokenStream {
    obf::str::s(input.into()).into()
}
#[cfg(feature = "s1")]
#[proc_macro]
pub fn ss(input: TokenStream) -> TokenStream {
    obf::str::ss(input.into()).into()
}
#[cfg(feature = "s1")]
#[proc_macro]
pub fn sss(input: TokenStream) -> TokenStream {
    obf::str::sss(input.into()).into()
}
#[cfg(feature = "s2")]
#[proc_macro]
pub fn s2(input: TokenStream) -> TokenStream {
    obf::str::s2(input.into()).into()
}
#[cfg(feature = "s2")]
#[proc_macro]
pub fn s2_add(input: TokenStream) -> TokenStream {
    obf::str::s2_add(input.into()).into()
}
#[cfg(feature = "s2")]
#[proc_macro]
pub fn s2_fmt(input: TokenStream) -> TokenStream {
    obf::str::s2_fmt(input.into()).into()
}
#[cfg(feature = "s1")]
#[proc_macro]
pub fn s_add(input: TokenStream) -> TokenStream {
    obf::str::s_add(input.into()).into()
}
#[cfg(feature = "s1")]
#[proc_macro]
pub fn s_fmt(input: TokenStream) -> TokenStream {
    obf::str::s_fmt(input.into()).into()
}

// ==========================================
// JUNK 垃圾指令块包装入口
// ==========================================



#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        // ...
    }
}
