#![allow(unused_qualifications)]
#![allow(clippy::similar_names)]
#![allow(unused)]
#![cfg(any(feature = "s1", feature = "s2"))]
// str运行时

pub trait Str:
    ::std::ops::Deref<Target = str>
    + ::std::convert::AsRef<str>
    + ::std::convert::AsRef<[u8]>
    + ::std::convert::AsRef<::std::path::Path>
    + ::std::fmt::Display
    + ::std::fmt::Debug
    + ::std::cmp::Eq
    + ::std::cmp::Ord
    + ::std::cmp::PartialEq<str>
    + for<'a> ::std::cmp::PartialEq<&'a str>
    + ::std::cmp::PartialEq<::std::string::String>
    + ::std::hash::Hash
    + ::std::borrow::Borrow<str>
    + Send
    + Sync
{
    fn as_str(&self) -> &str;
    fn as_bytes(&self) -> &[u8];
    fn concat<T: ::std::convert::AsRef<str>>(self, rhs: T) -> HeapStr
    where
        Self: Sized;

    #[inline(always)]
    fn len(&self) -> usize {
        self.as_str().len()
    }

    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.as_str().is_empty()
    }
}

pub struct HeapStr(pub ::std::vec::Vec<u8>);

pub struct StackStr<const N: usize>(pub [u8; N]);

impl HeapStr {
    #[inline(always)]
    pub fn new(src: impl ::std::convert::Into<Self>) -> Self {
        src.into()
    }

    #[inline(always)]
    pub fn into_bytes(self) -> ::std::vec::Vec<u8> {
        let this = ::std::mem::ManuallyDrop::new(self);
        unsafe { ::std::ptr::read(&this.0) }
    }

    #[inline(always)]
    pub fn into_string(self) -> ::std::string::String {
        ::std::string::String::from_utf8(self.into_bytes()).unwrap_or_default()
    }

    /// 当前已分配容量
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

impl Str for HeapStr {
    #[inline(always)]
    fn as_str(&self) -> &str {
        ::std::str::from_utf8(&self.0[..]).unwrap_or("")
    }

    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        &self.0[..]
    }

    #[inline(always)]
    fn concat<T: ::std::convert::AsRef<str>>(self, rhs: T) -> HeapStr {
        self + rhs
    }
}

impl<const N: usize> StackStr<N> {
    #[inline(always)]
    pub fn new(src: impl ::std::convert::Into<Self>) -> Self {
        src.into()
    }

    pub const CAPACITY: usize = N;

    #[inline(always)]
    pub fn into_bytes(self) -> [u8; N] {
        self.0
    }
}

impl<const N: usize> Str for StackStr<N> {
    #[inline(always)]
    fn as_str(&self) -> &str {
        ::std::str::from_utf8(&self.0[..])
            .unwrap_or("")
            .trim_end_matches('\0')
    }

    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        &self.0[..]
    }

    #[inline(always)]
    fn concat<T: ::std::convert::AsRef<str>>(self, rhs: T) -> HeapStr {
        self + rhs
    }
}

impl ::std::ops::Drop for HeapStr {
    #[inline(always)]
    fn drop(&mut self) {
        let cap = self.0.capacity();
        unsafe {
            __secure_wipe(self.0.as_mut_ptr(), cap);
        }
    }
}

impl<const N: usize> ::std::ops::Drop for StackStr<N> {
    #[inline(always)]
    fn drop(&mut self) {
        unsafe {
            __secure_wipe(self.0.as_mut_ptr(), N);
        }
    }
}

impl<const N: usize> ::std::convert::From<&mut [u8; N]> for HeapStr {
    #[inline(always)]
    fn from(arr: &mut [u8; N]) -> Self {
        let v = arr.to_vec();
        unsafe {
            __secure_wipe(arr.as_mut_ptr(), N);
        }
        HeapStr(v)
    }
}

impl ::std::convert::From<::std::vec::Vec<u8>> for HeapStr {
    #[inline(always)]
    fn from(v: ::std::vec::Vec<u8>) -> Self {
        HeapStr(v)
    }
}

impl<const N: usize> ::std::convert::From<::std::boxed::Box<[u8; N]>> for HeapStr {
    #[inline(always)]
    fn from(mut b: ::std::boxed::Box<[u8; N]>) -> Self {
        let v = b.to_vec();
        unsafe {
            __secure_wipe(b.as_mut_ptr(), N);
        }
        HeapStr(v)
    }
}

impl ::std::convert::From<::std::string::String> for HeapStr {
    #[inline(always)]
    fn from(s: ::std::string::String) -> Self {
        HeapStr(s.into_bytes())
    }
}

impl ::std::convert::From<&str> for HeapStr {
    #[inline(always)]
    fn from(s: &str) -> Self {
        HeapStr(s.as_bytes().to_vec())
    }
}

impl ::std::convert::From<(*mut u8, usize)> for HeapStr {
    #[inline(always)]
    fn from((ptr, len): (*mut u8, usize)) -> Self {
        if ptr.is_null() || len == 0 {
            return HeapStr::default();
        }
        // 调用方必须保证 ptr 和 len 指向有效的内存区域。
        let v = unsafe { ::std::slice::from_raw_parts(ptr, len) }.to_vec();
        unsafe {
            __secure_wipe(ptr, len);
        }
        HeapStr(v)
    }
}

impl ::std::convert::From<HeapStr> for ::std::string::String {
    #[inline(always)]
    fn from(s: HeapStr) -> Self {
        s.into_string()
    }
}

impl ::std::str::FromStr for HeapStr {
    type Err = ::std::convert::Infallible;
    #[inline(always)]
    fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(HeapStr::from(s))
    }
}

impl<const N: usize> ::std::convert::From<[u8; N]> for StackStr<N> {
    #[inline(always)]
    fn from(arr: [u8; N]) -> Self {
        StackStr(arr)
    }
}
impl<const N: usize> ::std::convert::From<&mut [u8; N]> for StackStr<N> {
    #[inline(always)]
    fn from(arr: &mut [u8; N]) -> Self {
        let s = StackStr(*arr);
        unsafe {
            __secure_wipe(arr.as_mut_ptr(), N);
        }
        s
    }
}

impl<const N: usize> ::std::convert::From<::std::vec::Vec<u8>> for StackStr<N> {
    fn from(mut v: ::std::vec::Vec<u8>) -> Self {
        let mut arr = [0u8; N];
        let len = v.len().min(N);
        arr[..len].copy_from_slice(&v[..len]);
        unsafe {
            __secure_wipe(v.as_mut_ptr(), v.capacity());
        }
        StackStr(arr)
    }
}

impl<const N: usize> ::std::convert::From<::std::boxed::Box<[u8; N]>> for StackStr<N> {
    fn from(mut b: ::std::boxed::Box<[u8; N]>) -> Self {
        let arr = *b;
        unsafe {
            __secure_wipe(b.as_mut_ptr(), N);
        }
        StackStr(arr)
    }
}
impl<const N: usize> ::std::convert::From<::std::string::String> for StackStr<N> {
    #[inline(always)]
    fn from(s: ::std::string::String) -> Self {
        Self::from(s.into_bytes())
    }
}

impl<const N: usize> ::std::convert::From<&str> for StackStr<N> {
    #[inline(always)]
    fn from(s: &str) -> Self {
        Self::from(s.as_bytes().to_vec())
    }
}
impl<const N: usize> ::std::convert::From<(*mut u8, usize)> for StackStr<N> {
    #[inline(always)]
    fn from((ptr, len): (*mut u8, usize)) -> Self {
        let mut arr = [0u8; N];
        if !ptr.is_null() && len > 0 {
            let copy_len = len.min(N);
            let slice = unsafe { ::std::slice::from_raw_parts(ptr, copy_len) };
            arr[..copy_len].copy_from_slice(slice);
            unsafe {
                __secure_wipe(ptr, len);
            }
        }
        StackStr(arr)
    }
}

impl<const N: usize> ::std::convert::From<StackStr<N>> for ::std::string::String {
    #[inline(always)]
    fn from(s: StackStr<N>) -> Self {
        s.as_str().to_owned()
    }
}

impl<const N: usize> ::std::str::FromStr for StackStr<N> {
    type Err = ::std::convert::Infallible;
    #[inline(always)]
    fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(StackStr::from(s))
    }
}

impl ::std::default::Default for HeapStr {
    #[inline(always)]
    fn default() -> Self {
        HeapStr(::std::vec::Vec::new())
    }
}

impl<const N: usize> ::std::default::Default for StackStr<N> {
    #[inline(always)]
    fn default() -> Self {
        StackStr([0u8; N])
    }
}

impl ::std::cmp::PartialEq for HeapStr {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

/// 跨容量比较:StackStr<N> 与 StackStr<M>(包含 N == M 的情形,即"自身比较")。
impl<const N: usize, const M: usize> ::std::cmp::PartialEq<StackStr<M>> for StackStr<N> {
    #[inline(always)]
    fn eq(&self, other: &StackStr<M>) -> bool {
        self.as_str() == other.as_str()
    }
}

/// 跨类型比较:HeapStr 与 StackStr<N>,两个方向都补上,保证 `==` 在两侧都能用。
impl<const N: usize> ::std::cmp::PartialEq<StackStr<N>> for HeapStr {
    #[inline(always)]
    fn eq(&self, other: &StackStr<N>) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<const N: usize> ::std::cmp::PartialEq<HeapStr> for StackStr<N> {
    #[inline(always)]
    fn eq(&self, other: &HeapStr) -> bool {
        self.as_str() == other.as_str()
    }
}

/// 跨类型比较:与拥有所有权的 String 比较(对称双向)。
impl ::std::cmp::PartialEq<::std::string::String> for HeapStr {
    #[inline(always)]
    fn eq(&self, other: &::std::string::String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl ::std::cmp::PartialEq<HeapStr> for ::std::string::String {
    #[inline(always)]
    fn eq(&self, other: &HeapStr) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<const N: usize> ::std::cmp::PartialEq<::std::string::String> for StackStr<N> {
    #[inline(always)]
    fn eq(&self, other: &::std::string::String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<const N: usize> ::std::cmp::PartialEq<StackStr<N>> for ::std::string::String {
    #[inline(always)]
    fn eq(&self, other: &StackStr<N>) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<T: ::std::convert::AsRef<str>> ::std::ops::Add<T> for HeapStr {
    type Output = HeapStr;
    #[inline(always)]
    fn add(mut self, rhs: T) -> Self::Output {
        self.0.extend_from_slice(rhs.as_ref().as_bytes());
        self
    }
}

impl<const N: usize, T: ::std::convert::AsRef<str>> ::std::ops::Add<T> for StackStr<N> {
    type Output = HeapStr;

    #[inline(always)]
    fn add(self, rhs: T) -> Self::Output {
        let mut v = ::std::vec::Vec::with_capacity(self.len() + rhs.as_ref().len());
        v.extend_from_slice(self.as_bytes());
        v.extend_from_slice(rhs.as_ref().as_bytes());

        HeapStr(v)
    }
}

impl<'a> ::std::convert::From<HeapStr> for ::std::borrow::Cow<'a, str> {
    #[inline(always)]
    fn from(s: HeapStr) -> Self {
        // 交出所有权给标准库的 String
        ::std::borrow::Cow::Owned(s.into_string())
    }
}

impl<'a, const N: usize> ::std::convert::From<StackStr<N>> for ::std::borrow::Cow<'a, str> {
    #[inline(always)]
    fn from(s: StackStr<N>) -> Self {
        // StackStr 转为 String 后交给 Cow
        ::std::borrow::Cow::Owned(::std::string::String::from(s))
    }
}

impl ::std::fmt::Write for HeapStr {
    #[inline(always)]
    fn write_str(&mut self, s: &str) -> ::std::fmt::Result {
        self.0.extend_from_slice(s.as_bytes());
        Ok(())
    }
}
impl HeapStr {
    #[inline(always)]
    pub fn append_display<D: ::std::fmt::Display>(mut self, val: D) -> Self {
        use ::std::fmt::Write;
        let _ = write!(self, "{}", val);
        self
    }
}

impl<const N: usize> StackStr<N> {
    #[inline(always)]
    pub fn append_display<D: ::std::fmt::Display>(self, val: D) -> HeapStr {
        let mut v = ::std::vec::Vec::with_capacity(self.len() + 16);
        v.extend_from_slice(self.as_bytes());
        let mut h = HeapStr(v);

        use ::std::fmt::Write;
        let _ = write!(h, "{}", val);
        h
    }
}
impl<const N: usize> ::std::convert::From<StackStr<N>> for HeapStr {
    #[inline(always)]
    fn from(s: StackStr<N>) -> Self {
        let v = s.as_str().as_bytes().to_vec();
        HeapStr(v)
    }
}

macro_rules! __impl_common_str_traits {
    ($name:ident $( <const $N:ident: usize> )?) => {
        impl $( <const $N: usize> )? ::std::clone::Clone for $name $( <$N> )? {
            #[inline(always)]
            fn clone(&self) -> Self {
                $name(self.0.clone())
            }
        }
        impl $( <const $N: usize> )? ::std::ops::Deref for $name $( <$N> )? {
            type Target = str;
            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                self.as_str()
            }
        }
        impl $( <const $N: usize> )? ::std::fmt::Display for $name $( <$N> )? {
            #[inline(always)]
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
        impl $( <const $N: usize> )? ::std::fmt::Debug for $name $( <$N> )? {
            #[inline(always)]
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::fmt::Debug::fmt(self.as_str(), f)
            }
        }
        impl $( <const $N: usize> )? ::std::convert::AsRef<str> for $name $( <$N> )? {
            #[inline(always)]
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
        impl $( <const $N: usize> )? ::std::convert::AsRef<[u8]> for $name $( <$N> )? {
            #[inline(always)]
            fn as_ref(&self) -> &[u8] {
                self.as_bytes()
            }
        }
        impl $( <const $N: usize> )? ::std::convert::AsRef<::std::path::Path> for $name $( <$N> )? {
            #[inline(always)]
            fn as_ref(&self) -> &::std::path::Path {
                ::std::path::Path::new(self.as_str())
            }
        }
        impl $( <const $N: usize> )? ::std::cmp::PartialEq<str> for $name $( <$N> )? {
            #[inline(always)]
            fn eq(&self, other: &str) -> bool {
                self.as_str() == other
            }
        }
        impl<'a, $( const $N: usize )?> ::std::cmp::PartialEq<&'a str> for $name $( <$N> )? {
            #[inline(always)]
            fn eq(&self, other: &&'a str) -> bool {
                self.as_str() == *other
            }
        }
        impl $( <const $N: usize> )? ::std::cmp::Eq for $name $( <$N> )? {}

        impl $( <const $N: usize> )? ::std::cmp::PartialOrd for $name $( <$N> )? {
            #[inline(always)]
            fn partial_cmp(&self, other: &Self) -> ::std::option::Option<::std::cmp::Ordering> {
                self.as_str().partial_cmp(other.as_str())
            }
        }
        impl $( <const $N: usize> )? ::std::cmp::Ord for $name $( <$N> )? {
            #[inline(always)]
            fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
                self.as_str().cmp(other.as_str())
            }
        }
        impl $( <const $N: usize> )? ::std::hash::Hash for $name $( <$N> )? {
            #[inline(always)]
            fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                self.as_str().hash(state)
            }
        }
        impl $( <const $N: usize> )? ::std::borrow::Borrow<str> for $name $( <$N> )? {
            #[inline(always)]
            fn borrow(&self) -> &str {
                self.as_str()
            }
        }
    };
}

__impl_common_str_traits!(HeapStr);
__impl_common_str_traits!(StackStr <const N: usize>);

#[cfg(test)]
mod tests {
    include::clean_include!("src/obf/rt/base.rs");
    include::clean_include!("src/obf/rt/str.rs");

    #[test]
    fn from_str_literal() {
        let h: HeapStr = "hello".into();
        let s: StackStr<8> = "hello".into();
        assert_eq!(h, "hello");
        assert_eq!(s, "hello");

        fn f(a: impl Str, b: impl Str) {
            println!("{}", a.concat(b));
        }
    }

    #[test]
    fn cross_type_eq() {
        let h: HeapStr = "same".into();
        let s8: StackStr<8> = "same".into();
        let s16: StackStr<16> = "same".into();
        let owned = String::from("same");

        assert_eq!(h, s8);
        assert_eq!(s8, h);
        assert_eq!(s8, s16);
        assert_eq!(h, owned);
        assert_eq!(owned, h);
        assert_eq!(s8, owned);
    }

    #[test]
    fn into_string_and_bytes() {
        let h: HeapStr = "convert-me".into();
        let owned: String = h.into();
        assert_eq!(owned, "convert-me");

        let s: StackStr<16> = "convert-me".into();
        let owned2: String = s.into();
        assert_eq!(owned2, "convert-me");
    }

    #[test]
    fn default_and_len() {
        let h = HeapStr::default();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);

        let s = StackStr::<8>::default();
        assert!(s.is_empty());
        assert_eq!(StackStr::<8>::CAPACITY, 8);
    }

    #[test]
    fn from_str_parse() {
        let h: HeapStr = "parsed".parse().unwrap();
        let s: StackStr<16> = "parsed".parse().unwrap();
        assert_eq!(h, s);
    }
    #[test]
    fn from_raw_ptr() {
        let mut source_data = b"secret_data".to_vec();
        let ptr = source_data.as_mut_ptr();
        let len = source_data.len();

        let h = HeapStr::new((ptr, len));
        assert_eq!(h.as_str(), "secret_data");

        assert_eq!(source_data, vec![0u8; 11]);

        let mut source_data2 = b"123".to_vec();
        let ptr2 = source_data2.as_mut_ptr();
        let len2 = source_data2.len();

        let s: StackStr<3> = StackStr::new((ptr2, len2));
        assert_eq!(s.as_str(), "123");
        // let s: StackStr<4> = StackStr::new((ptr2, len2));
        // assert_eq!(s.as_str(), "123");
        // let s: StackStr<2> = StackStr::new((ptr2, len2));
        // assert_eq!(s.as_str(), "12");

        assert_eq!(source_data2, vec![0u8; 3]);
    }
}

include::clean_include!("src/obf/rt/base.rs");
