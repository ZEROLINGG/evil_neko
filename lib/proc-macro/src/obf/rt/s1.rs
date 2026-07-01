#![cfg(feature = "s1")]
#![allow(unused)]



#[inline(never)]
pub unsafe fn __obf_1_decrypt_bytes(
    dst: *mut u8,
    src: *const u8,
    idx_start: usize,
    len: usize,
    key: u128,
) -> u128 {
    unsafe {
        let mut s = key;
        for i in 0..len {
            s = next_u128(s);
            *dst.add(i + idx_start) = __swap_u8_bits(*src.add(i) ^ s as u8 ^ i as u8, [6,2,0,5,1,7,3,4]);
        }
        s
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a(){}
}
include::clean_include!("src/obf/rt/base.rs");