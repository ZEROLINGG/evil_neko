//lib/src/lib.rs
#[cfg(any(feature = "ppty", feature = "pty"))]
pub mod shell;

#[cfg(feature = "sandbox")]
pub mod sandbox;
pub mod utils;

#[cfg(any(feature = "macros-junk", feature = "macros-buf", feature = "macros-s1", feature = "macros-s2", feature = "macros"))]
pub use libpm as pm;
#[cfg(feature = "macros-buf")]
pub use async_stream as __async_stream;
#[cfg(feature = "macros-buf")]
pub use futures_core as __futures_core;
#[cfg(feature = "macros-buf")]
pub use image;
#[cfg(feature = "macros-buf")]
pub use tokio_stream as __tokio_stream;



pub use tokio as __tokio;
pub use libpm::main;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
