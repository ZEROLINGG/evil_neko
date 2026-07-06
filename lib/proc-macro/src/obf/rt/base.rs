#![allow(unused_qualifications)]
#![allow(clippy::similar_names)]
#![allow(unused)]
// 由混淆宏注入的基础运行时
//lib/proc-macro/src/obf/rt/base.rs


// 垃圾填充生成时使用
pub fn seed() -> u128 {
    let now = ::std::time::Instant::now();
    let a = [0u8;1];
    let b = vec![0u8;1];
    let mut s = derive_u128(now) ^ derive_u128(a.as_ptr()) ^ derive_u128(b.as_ptr()) ^ derive_u128(seed as *const () as u128);
    for x in 0..32 {
        s = derive_u128(derive_u128(s) ^ derive_u128(x));
    }
    s
}

// 提供轻量的确定性随机数生成器，用于生成密钥序列
#[inline(never)]
pub fn derive_u128<T: ::std::hash::Hash>(value: T) -> u128 {
    struct __DeriveHasher(u128);
    impl __DeriveHasher { fn new() -> Self { __DeriveHasher(0x6c62272e07bb014262b821756295c58d) } }
    impl ::std::hash::Hasher for __DeriveHasher {
        fn finish(&self) -> u64 { self.0 as u64 }
        fn write(&mut self, bytes: &[u8]) {
            for &byte in bytes {
                self.0 ^= byte as u128;
                self.0 = self.0.wrapping_mul(0x1000000000000000000013b);
            }
        }
    }
    impl __DeriveHasher { fn finish_u128(&self) -> u128 { self.0 } }

    let mut hasher = __DeriveHasher::new();
    value.hash(&mut hasher);
    let result = hasher.finish_u128();

    let mut x = result;
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^= x >> 33;
    for _ in 0..32 {
        if x == 0 { x = 0x6c62272e07bb014262b821756295c58d; }
        x ^= x << 23;
        x ^= x >> 17;
        x ^= x << 26;
    }
    x
}

pub fn derive_string(mut seed: u128, len: usize) -> String {
    let mut result = String::with_capacity(len);
    for _ in 0..len {
        seed = next_u128(seed);
        let char_code = b'a' + (seed % 26) as u8;
        result.push(char_code as char);
    }
    result
}

pub fn next_u8<T: Into<u8>>(x: T) -> u8 { derive_u128(x.into()) as u8 }
pub fn next_u16<T: Into<u16>>(x: T) -> u16 { derive_u128(x.into()) as u16 }
pub fn next_u32<T: Into<u32>>(x: T) -> u32 { derive_u128(x.into()) as u32 }
pub fn next_u64<T: Into<u64>>(x: T) -> u64 { derive_u128(x.into()) as u64 }
pub fn next_u128<T: Into<u128>>(x: T) -> u128 { derive_u128(x.into()) }

// 安全擦除内存敏感数据
#[inline(never)]
pub unsafe fn __secure_wipe<T>(ptr: *mut T, len: usize) {
    if len == 0 || ptr.is_null() {
        return;
    }

    let byte_ptr = ptr as *mut u8;
    let total = len * ::std::mem::size_of::<T>();

    for i in 0..total {
        unsafe  { std::ptr::write_volatile(byte_ptr.add(i), 0x00); }
    }

    std::sync::atomic::compiler_fence(
        std::sync::atomic::Ordering::SeqCst
    );
}

pub trait ___RawPtr {
    fn into_u128(self) -> u128;
    fn from_u128(val: u128) -> Self;
}

impl<T> ___RawPtr for *const T {
    #[inline(always)]
    fn into_u128(self) -> u128 { self as u128 }
    #[inline(always)]
    fn from_u128(val: u128) -> Self { val as Self }
}

impl<T> ___RawPtr for *mut T {
    #[inline(always)]
    fn into_u128(self) -> u128 { self as u128 }
    #[inline(always)]
    fn from_u128(val: u128) -> Self { val as Self }
}


// 计算base - 1, 防ida xref
#[inline(never)]
#[cold]
#[doc(hidden)]
#[allow(unused_qualifications)]
pub fn __ptr_calc<P: ___RawPtr>(base: P, len: usize, seed: usize) -> P {
    #[inline(never)]
    #[doc(hidden)]
    #[allow(unused_qualifications)]
    fn ___ptr_calc_core(base: u128, len: usize, seed: usize) -> u128 {
        let mut t = [0u128; 8];
        for (i, val) in (seed as u128..=seed
            .checked_add(len % 4)
            .and_then(|v| v.checked_add(4))
            .unwrap_or(4) as u128).enumerate() {
            t[i] = val;
        }
        let mut t = ::std::hint::black_box(t);
        let tp = t.as_mut_ptr();
        let mut p = ::std::hint::black_box(base);
        let l = ::std::hint::black_box(len as u128);
        let s = ::std::hint::black_box(seed as u128);

        if match p.checked_add(l) { None => true, Some(v) => v > 0 } {
            unsafe {
                let tp2 = tp.add(1);
                *tp2 = l ^ s;
            }
            p = ::std::hint::black_box(p + t[1]);
        } else {
            p = ::std::hint::black_box(p.checked_div(t.len() as u128).unwrap())
        }

        p = ::std::hint::black_box(p ^ s);

        ::std::hint::black_box({
            let mut i = 0;
            for _t in &t {
                i += 1;
                if ::std::hint::black_box(s + l + p) < 2 {
                    p = ::std::hint::black_box(p & _t);
                } else if ::std::hint::black_box(_t.clone() == s) {
                    p = ::std::hint::black_box((p ^ s) - 1);
                } else if ::std::hint::black_box(_t ^ s == l) {
                    unsafe {
                        let tp3 = tp.add(i);
                        *tp3 = l;
                    }
                    continue;
                } else if ::std::hint::black_box(_t.clone() == p && _t.clone() == s && _t.clone() == l) {
                    p = ::std::hint::black_box((p * 3) - 1);
                } else {
                    break;
                }
                unsafe { *tp = p; }
            }
        });

        let r1 = ::std::hint::black_box(t[0]);
        let r2 = ::std::hint::black_box(t[1]);
        let r3 = ::std::hint::black_box(t[2] ^ l);
        let r = ::std::hint::black_box(r1 - r2 + r3);

        ::std::hint::black_box(r.clone())
    }
    let result = ___ptr_calc_core(base.into_u128(), len.into(), seed.into());
    P::from_u128(result)
}


#[inline(never)]
#[cold]
pub fn __swap_u8_bits(x: u8, table: [u8; 8]) -> u8 {
    let mut out = 0u8;
    for dst in 0..8 {
        let src = table[dst];
        let bit = (x >> src) & 1;
        out |= bit << dst;
    }
    out
}
#[inline(never)]
#[cold]
pub unsafe fn __random_table<T>(
    ptr: *mut T,
    len: usize,
    seed: u32,
) {
    if len <= 1 { return; }

    let mut s = seed;

    for i in (1..len).rev() {
        s = next_u32(s);
        let j = (s as usize) % (i + 1);
        unsafe {
            ::core::ptr::swap(ptr.add(i), ptr.add(j));
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a() {


        println!("{}",seed());
        println!("{}",seed());
        println!("{}",seed());
        println!("{}",seed());


        let mut x = 0_u8;
        for _ in 0..3 {
            x = next_u8(x);
            println!("{}",x);
        }
        let mut x = seed();
        let v = vec![1_u8, 2_u8, 3_u8];
        for i in 0..100 {
            x = next_u128(x);
            let p = unsafe {__ptr_calc(v.as_ptr(), v.len(), x as usize)};
            println!("i: {}, \tx: {}, \tp: {:?}, \tv.as_ptr(): {:?}", i, x, p, v.as_ptr());
            assert_eq!(unsafe { p.byte_add(1) }, v.as_ptr());

        }
    }
}