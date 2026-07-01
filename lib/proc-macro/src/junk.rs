
#![allow(unused)]
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use rand::prelude::*;
use rand::rng;


fn random_bytes_str(count: usize) -> String {
    let mut rng = rand::rng();
    let bytes: Vec<String> = (0..count)
        .map(|_| format!("0x{:02X}", rng.random::<u8>()))
        .collect();
    bytes.join(", ")
}


fn random_eater_opcode_bytes() -> String {
    let mut rng = rand::rng();
    let opcodes = [
        "0xE8",       
        "0xE9",       
        "0x0F, 0x84", 
        "0x0F, 0x85", 
    ];
    let chosen = opcodes.choose(&mut rng).unwrap();
    chosen.to_string()
}

pub fn fake_ret_eater_junk() -> TokenStream2 {
    let eater = format!(".byte {}", random_eater_opcode_bytes());
    let junk1 = format!(".byte {}", random_bytes_str(5));
    let junk2 = format!(".byte {}", random_bytes_str(5));

    quote! {

        #[cfg(target_arch = "x86_64")]
        unsafe {
            let mut _dummy = ::std::hint::black_box(4usize);
            core::arch::asm!(
                "stc",
                "jc 2f",
                ".byte 0xC3",
                #junk1,
                "pop rcx",
                "ret",
                #junk2,
                #eater,
                "2:",               
                inout("rcx") _dummy,
                options(nostack)
            );
        }

    }
}

pub fn jmp_over_eater_junk() -> TokenStream2 {
    let junk1 = format!(".byte {}", random_bytes_str(5));
    let junk2 = format!(".byte {}", random_bytes_str(5));
    let eater1 = format!(".byte {}", random_eater_opcode_bytes());
    let eater2 = format!(".byte {}", random_eater_opcode_bytes());
    let eater3 = format!(".byte {}", random_eater_opcode_bytes());

    quote! {

        #[cfg(target_arch = "x86_64")]
        unsafe {
            let mut _dummy = ::std::hint::black_box(1usize);
            core::arch::asm!(
                "jmp 2f",
                #junk1,
                #eater1,
                "2:",
                "jmp 3f",
                #junk2,
                #eater2,
                #eater3,
                "3:",
                inout("rax") _dummy,
                options(nostack)
            );
        }
    }
}

pub fn math_opaque_junk() -> TokenStream2 {
    let mut rng = rng();
    let eater = format!(".byte {}", random_eater_opcode_bytes());
    let junk1 = format!(".byte {}", random_bytes_str(4));
    let dummy = rng.random_range(3usize..256);

    quote! {

        #[cfg(target_arch = "x86_64")]
        unsafe {
        let mut _dummy = ::std::hint::black_box(#dummy);
            core::arch::asm!(
                "mov {tmp}, {val}",     
                "inc {tmp}",            
                "imul {tmp}, {val}",    
                "and {tmp}, 1",         
                "je 2f",                
                #eater,                 
                #junk1,
                "2:",                   
                val = inout(reg) _dummy,
                tmp = out(reg) _,       
                options(nostack)
            );
        }
    }
}
pub fn call_pop_junk() -> TokenStream2 {
    let eater = format!(".byte {}", random_eater_opcode_bytes());
    let junk_padding = format!(".byte {}", random_bytes_str(8));

    quote! {

        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!(
                "call 2f",              
                #eater,                 
                #junk_padding,          
                "2:",                   
                "add rsp, 8",           
                                        
                options(nostack)
            );
        }

    }
}

pub fn random_junk(x: Option<u8>) -> TokenStream2 {
    let fnc = vec![
        jmp_over_eater_junk,
        math_opaque_junk,
        fake_ret_eater_junk,
        call_pop_junk,
    ];
    let mut ts2 = TokenStream2::new();
    let mut rng = rand::rng();
    let count = x.unwrap_or(1);
    for _ in 0..=count {
        let junk = fnc.choose(&mut rng).unwrap()();
        ts2.extend(quote! { #junk; });
    }
    ts2
}
