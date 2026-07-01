#![allow(unused_qualifications)]
#![allow(unused)]


pub struct Bytes<const N: usize>(pub ::std::boxed::Box<[u8; N]>);

impl<const N: usize> Bytes<N> {
    /// 通用构造函数，接受任何实现了 Into<Bytes<N>> 的类型：
    /// - `[u8; N]`       — 直接装箱，零拷贝移动
    /// - `Vec<u8>`       — 截取前 N 字节，不足则补零
    /// - `Box<[u8; N]>`  — 接管所有权，真正零拷贝
    #[inline(always)]
    pub fn new(src: impl ::std::convert::Into<Self>) -> Self {
        src.into()
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        &self.0[..]
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.0[..]
    }
}

// ── From impls ───────────────────────────────────────────────────────

// 来源 1: [u8; N] — 直接装箱，零拷贝移动
impl<const N: usize> ::std::convert::From<[u8; N]> for Bytes<N> {
    #[inline(always)]
    fn from(arr: [u8; N]) -> Self {
        Bytes(::std::boxed::Box::new(arr))
    }
}

// 来源 2: Vec<u8> — 截取前 N 字节，不足则补零
impl<const N: usize> ::std::convert::From<::std::vec::Vec<u8>> for Bytes<N> {
    #[inline(always)]
    fn from(v: ::std::vec::Vec<u8>) -> Self {
        let mut arr = [0u8; N];
        let len = v.len().min(N);
        arr[..len].copy_from_slice(&v[..len]);
        Bytes(::std::boxed::Box::new(arr))
    }
}

// 来源 3: Box<[u8; N]> — 接管所有权，真正零拷贝
impl<const N: usize> ::std::convert::From<::std::boxed::Box<[u8; N]>> for Bytes<N> {
    #[inline(always)]
    fn from(b: ::std::boxed::Box<[u8; N]>) -> Self {
        Bytes(b)
    }
}

// ── Trait impls ──────────────────────────────────────────────────────

// 1. Deref: 自动解引用为 &[u8]
impl<const N: usize> ::std::ops::Deref for Bytes<N> {
    type Target = [u8];
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0[..]
    }
}

// 2. DerefMut: 支持就地修改解密后的字节
impl<const N: usize> ::std::ops::DerefMut for Bytes<N> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0[..]
    }
}

// 3. Debug: 支持 {:?} 打印为 HEX 数组
impl<const N: usize> ::std::fmt::Debug for Bytes<N> {
    #[inline(always)]
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        ::std::fmt::Debug::fmt(&self.0[..], f)
    }
}

// 4. AsRef 家族
impl<const N: usize> ::std::convert::AsRef<[u8]> for Bytes<N> {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] { &self.0[..] }
}

// 5. PartialEq & Eq
impl<const N: usize> ::std::cmp::PartialEq for Bytes<N> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool { self.0[..] == other.0[..] }
}
impl<const N: usize> ::std::cmp::PartialEq<[u8]> for Bytes<N> {
    #[inline(always)]
    fn eq(&self, other: &[u8]) -> bool { &self.0[..] == other }
}
impl<const N: usize> ::std::cmp::Eq for Bytes<N> {}

// 6. Clone: 显式克隆
impl<const N: usize> ::std::clone::Clone for Bytes<N> {
    #[inline(always)]
    fn clone(&self) -> Self {
        let mut new_box: ::std::boxed::Box<[u8; N]> = ::std::vec![0u8; N].into_boxed_slice().try_into().unwrap_or_else(|_| unsafe { ::std::hint::unreachable_unchecked() });
        new_box.copy_from_slice(&self.0[..]);
        Bytes(new_box)
    }
}

// 7. 内存安全擦除: 依靠常量泛型 N 动态安全擦除 (防内存 Dump)
impl<const N: usize> ::std::ops::Drop for Bytes<N> {
    #[inline(always)]
    fn drop(&mut self) {
        unsafe {
            let ptr = self.0[..].as_mut_ptr();
            for i in 0..N {
                ::std::ptr::write_volatile(ptr.add(i), 0);
            }
        }
    }
}