// lib/proc-macro/src/obf.rs
#[cfg(any(feature = "s1", feature = "s2"))]
pub(crate) mod str;
#[cfg(feature = "buf")]
mod split_bytes;
pub mod rt;
#[cfg(feature = "buf")]
pub(crate) mod buf;

