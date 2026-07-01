#![allow(unused_qualifications)]
#![allow(clippy::similar_names)]
#![allow(unused)]
#![cfg(feature = "buf")]
// lib/proc-macro/src/obf/rt/buf.rs
// Buffer运行时



#[allow(unused)]
use tokio_stream::StreamExt;

/// 装箱的异步结果类型,绑定 Send,方便在多线程 tokio runtime 上调度
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

pub struct Lazy<F>(pub F, pub usize);

pub enum Chunk<'a> {
    Slice(&'a [u8]),
    Vec(Vec<u8>),
    /// (计算闭包, 该数据源声明的长度)
    /// 闭包按 idx 返回一个 Future<Output = u8>,装箱后统一成 BoxFuture
    Computed((Box<dyn Fn(usize) -> BoxFuture<'a, u8> + Send + Sync + 'a>, usize)),
    /// 惰性加载：首次访问时调用 loader,结果缓存到 cache
    LazyVec {
        loader: Box<dyn Fn() -> BoxFuture<'a, Vec<u8>> + Send + Sync + 'a>,
        len: usize,
        cache: tokio::sync::OnceCell<Vec<u8>>,
    },
}

impl<'a> Chunk<'a> {
    pub fn len(&self) -> usize {
        match self {
            Chunk::Slice(slice) => slice.len(),
            Chunk::Vec(vec) => vec.len(),
            Chunk::Computed((_, len)) => *len,
            Chunk::LazyVec { len, .. } => *len,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub async fn get(&self, idx: usize) -> Option<u8> {
        if idx >= self.len() {
            return None;
        }
        match self {
            Chunk::Slice(slice) => slice.get(idx).copied(),
            Chunk::Vec(vec) => vec.get(idx).copied(),
            Chunk::Computed((f, _)) => Some(f(idx).await),
            Chunk::LazyVec { loader, len, cache } => {
                let vec = cache
                    .get_or_init(|| async {
                        let v = loader().await;
                        debug_assert_eq!(
                            v.len(),
                            *len,
                            "LazyVec loader returned {} bytes, declared len is {}",
                            v.len(),
                            len
                        );
                        v
                    })
                    .await;
                vec.get(idx).copied()
            }
        }
    }

    /// 批量读取块内 [read_start, read_end) 并追加到 result
    async fn read_into(&self, result: &mut Vec<u8>, read_start: usize, read_end: usize) {
        match self {
            Chunk::Slice(slice) => {
                result.extend_from_slice(slice.get(read_start..read_end).expect("slice bounds"));
            }
            Chunk::Vec(vec) => {
                result.extend_from_slice(vec.get(read_start..read_end).expect("vec bounds"));
            }
            Chunk::Computed((f, _)) => {
                for local_idx in read_start..read_end {
                    result.push(f(local_idx).await);
                }
            }
            Chunk::LazyVec { loader, len, cache } => {
                let vec = cache
                    .get_or_init(|| async {
                        let v = loader().await;
                        debug_assert_eq!(
                            v.len(),
                            *len,
                            "LazyVec loader returned {} bytes, declared len is {}",
                            v.len(),
                            len
                        );
                        v
                    })
                    .await;
                result.extend_from_slice(vec.get(read_start..read_end).expect("lazy vec bounds"));
            }
        }
    }
}

// ─── From impls ──────────────────────────────────────────────────────────────

impl<'a> From<&'a [u8]> for Chunk<'a> {
    fn from(slice: &'a [u8]) -> Self {
        Chunk::Slice(slice)
    }
}

impl<'a, const N: usize> From<&'a [u8; N]> for Chunk<'a> {
    fn from(arr: &'a [u8; N]) -> Self {
        Chunk::Slice(arr.as_slice())
    }
}

impl<'a> From<Vec<u8>> for Chunk<'a> {
    fn from(vec: Vec<u8>) -> Self {
        Chunk::Vec(vec)
    }
}

impl<'a, F, Fut> From<(F, usize)> for Chunk<'a>
where
    F: Fn(usize) -> Fut + Send + Sync + 'a,
    Fut: std::future::Future<Output = u8> + Send + 'a,
{
    fn from(computed: (F, usize)) -> Self {
        let f = computed.0;
        let boxed: Box<dyn Fn(usize) -> BoxFuture<'a, u8> + Send + Sync + 'a> =
            Box::new(move |idx| Box::pin(f(idx)));
        Chunk::Computed((boxed, computed.1))
    }
}

impl<'a, F, Fut> From<Lazy<F>> for Chunk<'a>
where
    F: Fn() -> Fut + Send + Sync + 'a,
    Fut: std::future::Future<Output = Vec<u8>> + Send + 'a,
{
    fn from(lazy: Lazy<F>) -> Self {
        let f = lazy.0;
        let boxed: Box<dyn Fn() -> BoxFuture<'a, Vec<u8>> + Send + Sync + 'a> =
            Box::new(move || Box::pin(f()));
        Chunk::LazyVec {
            len: lazy.1,
            loader: boxed,
            cache: tokio::sync::OnceCell::new(),
        }
    }
}

impl<'a, F, Fut> From<(F, usize, ())> for Chunk<'a>
where
    F: Fn() -> Fut + Send + Sync + 'a,
    Fut: std::future::Future<Output = Vec<u8>> + Send + 'a,
{
    fn from(lazy_tuple: (F, usize, ())) -> Self {
        let f = lazy_tuple.0;
        let boxed: Box<dyn Fn() -> BoxFuture<'a, Vec<u8>> + Send + Sync + 'a> =
            Box::new(move || Box::pin(f()));
        Chunk::LazyVec {
            len: lazy_tuple.1,
            loader: boxed,
            cache: tokio::sync::OnceCell::new(),
        }
    }
}

// ─── Buffer ───────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct Buffer<'a> {
    chunks: Vec<Chunk<'a>>,
    end_indices: Vec<usize>,
    total_len: usize,
}

impl<'a> Buffer<'a> {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            end_indices: Vec::new(),
            total_len: 0,
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            chunks: Vec::with_capacity(cap),
            end_indices: Vec::with_capacity(cap),
            total_len: 0,
        }
    }

    #[inline]
    fn chunk_start(&self, chunk_idx: usize) -> usize {
        if chunk_idx == 0 {
            0
        } else {
            self.end_indices[chunk_idx - 1]
        }
    }

    /// push 本身不涉及任何异步求值(长度在构造 Chunk 时已经声明好),
    /// 所以保持同步签名不变
    pub fn push<C: Into<Chunk<'a>>>(&mut self, chunk: C) {
        let chunk = chunk.into();
        if chunk.is_empty() {
            return;
        }
        self.total_len += chunk.len();
        self.end_indices.push(self.total_len);
        self.chunks.push(chunk);
        debug_assert_eq!(self.chunks.len(), self.end_indices.len());
    }

    pub fn len(&self) -> usize {
        self.total_len
    }

    pub fn is_empty(&self) -> bool {
        self.total_len == 0
    }

    pub async fn get(&self, idx: usize) -> Option<u8> {
        if idx >= self.total_len {
            return None;
        }
        let chunk_idx = match self.end_indices.binary_search(&idx) {
            Ok(i) => i + 1,
            Err(i) => i,
        };
        let chunk = self.chunks.get(chunk_idx)?;
        let start = self.chunk_start(chunk_idx);
        chunk.get(idx - start).await
    }

    pub async fn read(&self, idx_start: usize, idx_end: usize) -> Option<Vec<u8>> {
        if idx_start > idx_end || idx_end > self.total_len {
            return None;
        }
        let target_len = idx_end - idx_start;
        if target_len == 0 {
            return Some(Vec::new());
        }

        let mut result = Vec::with_capacity(target_len);

        let mut chunk_idx = match self.end_indices.binary_search(&idx_start) {
            Ok(i) => i + 1,
            Err(i) => i,
        };

        let mut cur_idx = idx_start;
        while cur_idx < idx_end {
            let chunk = &self.chunks[chunk_idx];
            let c_start = self.chunk_start(chunk_idx);
            let c_end = self.end_indices[chunk_idx];

            let read_start = cur_idx - c_start;
            let read_end = std::cmp::min(idx_end, c_end) - c_start;
            let read_len = read_end - read_start;

            chunk.read_into(&mut result, read_start, read_end).await;

            cur_idx += read_len;
            chunk_idx += 1;
        }

        Some(result)
    }

    pub fn iter<'b>(&'b self) -> std::pin::Pin<Box<dyn futures_core::Stream<Item = u8> + Send + 'b>> {
        Box::pin(async_stream::stream! {
            for chunk in &self.chunks {
                let mut buf = Vec::with_capacity(chunk.len());
                chunk.read_into(&mut buf, 0, chunk.len()).await;
                for b in buf {
                    yield b;
                }
            }
        })
    }


    pub fn iter_block<'b>(&'b self, block_size: usize) -> std::pin::Pin<Box<dyn futures_core::Stream<Item = Vec<u8>> + Send + 'b>> {
        Box::pin(async_stream::stream! {
            let mut current_idx = 0usize;
            while current_idx < self.total_len {
                let next_idx = std::cmp::min(current_idx + block_size, self.total_len);
                if let Some(block) = self.read(current_idx, next_idx).await {
                    yield block;
                }
                current_idx = next_idx;
            }
        })
    }

    pub async fn to_vec(&self) -> Vec<u8> {
        if self.is_empty() {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(self.total_len);

        for chunk in &self.chunks {
            chunk.read_into(&mut result, 0, chunk.len()).await;
        }

        result
    }
}

impl<'a> Drop for Chunk<'a> {
    fn drop(&mut self) {
        match self {
            Chunk::Vec(v) => {
                unsafe { __secure_wipe(v.as_mut_ptr(), v.capacity()); }
            }
            Chunk::LazyVec { cache, .. } => {
                if let Some(v) = cache.get_mut() {
                    unsafe  { __secure_wipe(v.as_mut_ptr(), v.capacity()); }
                }
            }
            _ => {}
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[allow(static_mut_refs)]
    async fn test_basic() {
        static mut DATA: [u8; 12] = *b"wwwwwwwwwwww";

        let mut buf = Buffer::new();
        // LazyVec: F: Fn() -> Fut<Output = Vec<u8>>
        buf.push((|| async { vec![1u8, 2, 3, 4] }, 4, ()));
        unsafe {
            buf.push(&DATA);
        }

        assert_eq!(buf.get(0).await, Some(1));
        assert_eq!(buf.get(3).await, Some(4));
        assert_eq!(buf.get(4).await, Some(b'w'));


        let mut collected = Vec::new();
        let mut stream = buf.iter();
        while let Some(x) = stream.next().await {
            collected.push(x);
        }
        println!("{:?}", collected);
        assert_eq!(collected, vec![1, 2, 3, 4, b'w', b'w', b'w', b'w', b'w', b'w', b'w', b'w', b'w', b'w', b'w', b'w']);

        let mut buf2 = Buffer::new();
        buf2.push((async move |idx: usize| { (idx as u8) * 2 }, 5));
        let v = buf2.read(0, 5).await.unwrap();
        assert_eq!(v, vec![0, 2, 4, 6, 8]);

        // iter_block
        let blocks: Vec<Vec<u8>> = buf.iter_block(3).collect().await;
        assert_eq!(blocks.concat(), buf.to_vec().await);
    }
}
include::clean_include!("src/obf/rt/base.rs");  // 引入基础运行时