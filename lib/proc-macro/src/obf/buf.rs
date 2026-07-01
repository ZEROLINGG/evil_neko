use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
pub fn buf(input: TokenStream2) -> TokenStream2 {
    let parsed_input = match syn::parse2::<crate::parse::BytesInput>(input) {
        Ok(parsed) => parsed,
        Err(err) => return err.to_compile_error(),
    };
    let ident = syn::Ident::new("__buffer", proc_macro2::Span::call_site());

    let ts = crate::obf::split_bytes::split_bytes(&*parsed_input.bytes, ident.clone(), 50);
    
    quote! {{
        unsafe {
            let mut #ident: Buffer = Buffer::new();
            #ts
            #ident
        }
    }}

}