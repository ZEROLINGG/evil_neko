// //lib/src/_lib_hash.rs
// use hex;
// 
// // ============================ Trait 定义 ============================
// 
// pub trait Hasher {
//     /// 返回二进制摘要（推荐用于内部使用）
//     fn digest_vec<T: AsRef<[u8]>>(input: T) -> Vec<u8>;
// 
//     /// 返回小写十六进制字符串（32或64字符）
//     fn digest_hex<T: AsRef<[u8]>>(input: T) -> String;
// }
// 
// // ============================ 宏实现 ============================
// 
// macro_rules! impl_hasher {
//     ($struct:ty, $hasher_type:ty, $output_len:expr) => {
//         impl Hasher for $struct {
//             fn digest_vec<T: AsRef<[u8]>>(input: T) -> Vec<u8> {
//                 use sha2::Digest;
//                 let mut hasher = <$hasher_type>::new();
//                 hasher.update(input.as_ref());
//                 hasher.finalize().to_vec()
//             }
// 
//             fn digest_hex<T: AsRef<[u8]>>(input: T) -> String {
//                 hex::encode(Self::digest_vec(input))
//             }
//         }
//     };
// }
// 
// 
// pub struct Sha256;
// impl_hasher!(Sha256, sha2::Sha256, 32);
// 
// pub struct Sha512;
// impl_hasher!(Sha512, sha2::Sha512, 64);
// 
// pub struct Sha512_256;
// impl_hasher!(Sha512_256, sha2::Sha512_256, 32);
// 
// pub struct Blake3;
// impl Hasher for Blake3 {
//     fn digest_vec<T: AsRef<[u8]>>(input: T) -> Vec<u8> {
//         blake3::hash(input.as_ref()).as_bytes().to_vec()
//     }
// 
//     fn digest_hex<T: AsRef<[u8]>>(input: T) -> String {
//         blake3::hash(input.as_ref()).to_hex().to_string()
//     }
// }
// 
// 
// #[cfg(test)]
// mod tests {
//     use super::*;
// 
//     #[test]
//     fn test_sha256() {
//         println!("{}",Sha512_256::digest_hex("Hello, World!"));
//     }
// }