use anyhow::{anyhow, bail, Result};

pub trait Cipher {
    fn encrypt<T: AsRef<[u8]>>(key: &[u8], plaintext: T) -> Result<Vec<u8>>;
    fn decrypt<T: AsRef<[u8]>>(key: &[u8], ciphertext: T) -> Result<Vec<u8>>;
}

// ─── GCM 模式辅助宏（nonce 12 字节前置） ─────────────────────────────────────

/// GCM 加密输出格式：[ nonce (12 B) | ciphertext + tag (plaintext.len() + 16 B) ]
macro_rules! impl_gcm_cipher {
    ($struct:ty, $cipher_type:ty, $key_len:expr) => {
        impl Cipher for $struct {
            fn encrypt<T: AsRef<[u8]>>(key: &[u8], plaintext: T) -> Result<Vec<u8>> {
                use aes_gcm::{
                    aead::{Aead, AeadCore, KeyInit, OsRng},
                    Key,
                };

                if key.len() != $key_len {
                    bail!("Invalid key length: expected {}, got {}", $key_len, key.len());
                }

                let key = Key::<$cipher_type>::from_slice(key);
                let cipher = <$cipher_type>::new(key);
                let nonce = <$cipher_type>::generate_nonce(OsRng);

                let ciphertext = cipher
                    .encrypt(&nonce, plaintext.as_ref())
                    .map_err(|_| anyhow!("Encryption failed"))?;

                let mut output = Vec::with_capacity(12 + ciphertext.len());
                output.extend_from_slice(&nonce);
                output.extend_from_slice(&ciphertext);
                Ok(output)
            }

            fn decrypt<T: AsRef<[u8]>>(key: &[u8], ciphertext: T) -> Result<Vec<u8>> {
                use aes_gcm::{
                    aead::{Aead, KeyInit},
                    Key, Nonce,
                };

                let ciphertext = ciphertext.as_ref();
                if key.len() != $key_len {
                    bail!("Invalid key length: expected {}, got {}", $key_len, key.len());
                }
                if ciphertext.len() < 12 {
                    bail!("Ciphertext too short to contain nonce");
                }

                let key = Key::<$cipher_type>::from_slice(key);
                let cipher = <$cipher_type>::new(key);
                let (nonce_bytes, data) = ciphertext.split_at(12);
                let nonce = Nonce::from_slice(nonce_bytes);

                cipher
                    .decrypt(nonce, data)
                    .map_err(|_| anyhow!("Decryption failed: auth tag mismatch or invalid data"))
            }
        }
    };
}

// ─── GCM-SIV 模式辅助宏（nonce 12 字节前置） ─────────────────────────────────
//
// GCM-SIV 与 GCM 输出格式相同：[ nonce (12 B) | ciphertext + tag (plaintext.len() + 16 B) ]
// 优势：nonce 可复用而不会泄露明文，适合难以保证 nonce 唯一的场景。
//
macro_rules! impl_gcm_siv_cipher {
    ($struct:ty, $cipher_type:ty, $key_len:expr) => {
        impl Cipher for $struct {
            fn encrypt<T: AsRef<[u8]>>(key: &[u8], plaintext: T) -> Result<Vec<u8>> {
                use aes_gcm_siv::{
                    aead::{Aead, AeadCore, KeyInit, OsRng},
                    Key,
                };

                if key.len() != $key_len {
                    bail!("Invalid key length: expected {}, got {}", $key_len, key.len());
                }

                let key = Key::<$cipher_type>::from_slice(key);
                let cipher = <$cipher_type>::new(key);
                let nonce = <$cipher_type>::generate_nonce(OsRng);

                let ciphertext = cipher
                    .encrypt(&nonce, plaintext.as_ref())
                    .map_err(|_| anyhow!("Encryption failed"))?;

                let mut output = Vec::with_capacity(12 + ciphertext.len());
                output.extend_from_slice(&nonce);
                output.extend_from_slice(&ciphertext);
                Ok(output)
            }

            fn decrypt<T: AsRef<[u8]>>(key: &[u8], ciphertext: T) -> Result<Vec<u8>> {
                use aes_gcm_siv::{
                    aead::{Aead, KeyInit},
                    Key, Nonce,
                };

                let ciphertext = ciphertext.as_ref();
                if key.len() != $key_len {
                    bail!("Invalid key length: expected {}, got {}", $key_len, key.len());
                }
                if ciphertext.len() < 12 {
                    bail!("Ciphertext too short to contain nonce");
                }

                let key = Key::<$cipher_type>::from_slice(key);
                let cipher = <$cipher_type>::new(key);
                let (nonce_bytes, data) = ciphertext.split_at(12);
                let nonce = Nonce::from_slice(nonce_bytes);

                cipher
                    .decrypt(nonce, data)
                    .map_err(|_| anyhow!("Decryption failed: auth tag mismatch or invalid data"))
            }
        }
    };
}

// ─── ChaCha20 系列辅助宏 ──────────────────────────────────────────────────────
//
// 统一处理 ChaCha20-Poly1305（nonce 12 B）和 XChaCha20-Poly1305（nonce 24 B）。
// 输出格式：[ nonce ($nonce_len B) | ciphertext + tag (plaintext.len() + 16 B) ]
//
// 两者均使用 256-bit（32 字节）密钥。
// XChaCha20 的 24 字节 nonce 可安全随机生成，nonce 碰撞概率可忽略不计。
//
macro_rules! impl_chacha_cipher {
    ($struct:ty, $cipher_type:ty, $nonce_len:expr) => {
        impl Cipher for $struct {
            fn encrypt<T: AsRef<[u8]>>(key: &[u8], plaintext: T) -> Result<Vec<u8>> {
                use chacha20poly1305::{
                    aead::{Aead, AeadCore, KeyInit, OsRng},
                    Key,
                };

                if key.len() != 32 {
                    bail!("Invalid key length: expected 32, got {}", key.len());
                }

                let key = Key::from_slice(key);
                let cipher = <$cipher_type>::new(key);
                let nonce = <$cipher_type>::generate_nonce(OsRng);

                let ciphertext = cipher
                    .encrypt(&nonce, plaintext.as_ref())
                    .map_err(|_| anyhow!("Encryption failed"))?;

                let mut output = Vec::with_capacity($nonce_len + ciphertext.len());
                output.extend_from_slice(&nonce);
                output.extend_from_slice(&ciphertext);
                Ok(output)
            }

            fn decrypt<T: AsRef<[u8]>>(key: &[u8], ciphertext: T) -> Result<Vec<u8>> {
                use chacha20poly1305::{
                    aead::{generic_array::GenericArray, Aead, AeadCore, KeyInit},
                    Key,
                };

                let ciphertext = ciphertext.as_ref();
                if key.len() != 32 {
                    bail!("Invalid key length: expected 32, got {}", key.len());
                }
                if ciphertext.len() < $nonce_len {
                    bail!("Ciphertext too short to contain nonce");
                }

                let key = Key::from_slice(key);
                let cipher = <$cipher_type>::new(key);
                let (nonce_bytes, data) = ciphertext.split_at($nonce_len);
                // ↓ 从 AeadCore 关联类型 NonceSize 推导长度，ChaCha→U12，XChaCha→U24
                let nonce = GenericArray::<u8, <$cipher_type as AeadCore>::NonceSize>::from_slice(
                    nonce_bytes,
                );

                cipher
                    .decrypt(nonce, data)
                    .map_err(|_| anyhow!("Decryption failed: auth tag mismatch or invalid data"))
            }
        }
    };
}

// ─── 具体实现 ─────────────────────────────────────────────────────────────────

pub struct Aes128Gcm;
impl_gcm_cipher!(Aes128Gcm, aes_gcm::Aes128Gcm, 16);

pub struct Aes256Gcm;
impl_gcm_cipher!(Aes256Gcm, aes_gcm::Aes256Gcm, 32);

pub struct Aes128GcmSiv;
impl_gcm_siv_cipher!(Aes128GcmSiv, aes_gcm_siv::Aes128GcmSiv, 16);

pub struct Aes256GcmSiv;
impl_gcm_siv_cipher!(Aes256GcmSiv, aes_gcm_siv::Aes256GcmSiv, 32);

/// ChaCha20-Poly1305，nonce 12 字节，密钥 32 字节
pub struct ChaCha20Poly1305;
impl_chacha_cipher!(ChaCha20Poly1305, chacha20poly1305::ChaCha20Poly1305, 12);

/// XChaCha20-Poly1305，nonce 24 字节，密钥 32 字节
/// nonce 可安全随机生成，适合高并发/分布式场景
pub struct XChaCha20Poly1305;
impl_chacha_cipher!(XChaCha20Poly1305, chacha20poly1305::XChaCha20Poly1305, 24);

// ─── 单元测试 ─────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"\
!function(o,t){''object''==typeof exports&&''object''==typeof module?module.exports=t():''function''==typeof define&&define.amd?define([],t):''object''==typeof exports?exports[''family-aa686e0e9e4d122550f8'']=t():o[''family-aa686e0e9e4d122550f8'']=t()}(window,function(){return function(o){var t={};function q(n){if(t[n])return t[n].exports;var e=t[n]={i:n,l:!1,exports:{}};return o[n].call(e.exports,e,e.exports,q),e.l=!0,e.exports}return q.m=o,q.c=t,q.d=function(o,t,n){q.o(o,t)||Object.defineProperty(o,t,{enumerable:!0,get:n})},q.r=function(o){''undefined''!=typeof Symbol&&Symbol.toStringTag&&Object.defineProperty(o,Symbol.toStringTag,{value:''Module''}),Object.defineProperty(o,''__esModule'',{value:!0})},q.t=function(o,t){if(1&t&&(o=q(o)),8&t)return o;if(4&t&&''object''==typeof o&&o&&o.__esModule)return o;var n=Object.create(null);if(q.r(n),Object.defineProperty(n,''default'',{enumerable:!0,value:o}),2&t&&''string''!=typeof o)for(var e in o)q.d(n,e,function(t){return o[t]}.bind(null,e));return n},q.n=function(o){var t=o&&o.__esModule?function(){return o.default}:function(){return o};return q.d(t,''a'',t),t},q.o=function(o,t){return Object.prototype.hasOwnProperty.call(o,t)},q.p=''/'',q(q.s=62)}({0:function(o,t,q){''use strict'';(function(o,n){vare;q.d(t,''a'',function(){return basicScroll}),function(t){''object''==typeof exports&&void 0!==o?o.exports=t():''function''==typeof define&&q(45)?define([],t):(''undefined''!=typeof window?window:void 0!==n?n:''undefined''!=typeof self?self:this).basicScroll=t()}(function(){return function o(t,q,n){function r(U,a){if(!q[U]){if(!t[U]){if(!a&&''function''==typeof e&&e)return e(U,!0);if(V)return V(U,!0);var i=new Error(''Cannot find module '''+U+''''');throwi.code=''MODULE_NOT_FOUND'',i}var p=q[U]={exports:{}};t[U][0].call(p.exports,function(o){return r(t[U][1][o]||o)},p,p.exports,o,t,q,n)}return q[U].exports}for(var V=''function''==typeof e&&e,U=0;U<n.length;U++)r(n[U]);return r}({1:[function(o,t,q){t.exports=function(o){var t=2.5949095;return(o*=2)<1?o*o*((t+1)*o-t)*.5:.5*((o-=2)*o*((t+1)*o+t)+2)}},{}],2:[function(o,t,q){t.exports=function(o){var t=1.70158;return o*o*((t+1)*o-t)}},{}],3:[function(o,t,q){t.exports=function(o){var t=1.70158;return--o*o*((t+1)*o+t)+1}},{}],4:[function(o,t,q){var n=o(''./bounce-out'');t.ex
        ";

    fn round_trip<C: Cipher>(label: &str, key: &[u8]) {
        let encrypted = C::encrypt(key, SAMPLE).expect("encrypt failed");
        let decrypted = C::decrypt(key, &encrypted).expect("decrypt failed");
        assert_eq!(decrypted, SAMPLE, "{label}: round-trip mismatch");
        println!(
            "{label}: {} -> {} bytes ({:.1}%)",
            SAMPLE.len(),
            encrypted.len(),
            encrypted.len() as f64 / SAMPLE.len() as f64 * 100.0
        );
    }

    fn wrong_key_returns_err<C: Cipher>(label: &str, key: &[u8], bad_key: &[u8]) {
        let encrypted = C::encrypt(key, SAMPLE).expect("encrypt failed");
        let result = C::decrypt(bad_key, &encrypted);
        assert!(result.is_err(), "{label}: expected Err with wrong key");
    }

    // ── AES-128-GCM ──────────────────────────────────────────────────────────
    #[test]
    fn test_aes128gcm_round_trip() {
        round_trip::<Aes128Gcm>("aes-128-gcm", &[0x42u8; 16]);
    }

    // ── AES-256-GCM ──────────────────────────────────────────────────────────
    #[test]
    fn test_aes256gcm_round_trip() {
        round_trip::<Aes256Gcm>("aes-256-gcm", &[0x7Eu8; 32]);
    }

    // ── AES-128-GCM-SIV ──────────────────────────────────────────────────────
    #[test]
    fn test_aes128gcmsiv_round_trip() {
        round_trip::<Aes128GcmSiv>("aes-128-gcm-siv", &[0x42u8; 16]);
    }

    #[test]
    fn test_aes128gcmsiv_wrong_key() {
        wrong_key_returns_err::<Aes128GcmSiv>("aes-128-gcm-siv", &[0x42u8; 16], &[0x00u8; 16]);
    }

    // ── AES-256-GCM-SIV ──────────────────────────────────────────────────────
    #[test]
    fn test_aes256gcmsiv_round_trip() {
        round_trip::<Aes256GcmSiv>("aes-256-gcm-siv", &[0x7Eu8; 32]);
    }

    #[test]
    fn test_aes256gcmsiv_wrong_key() {
        wrong_key_returns_err::<Aes256GcmSiv>("aes-256-gcm-siv", &[0x7Eu8; 32], &[0x00u8; 32]);
    }

    // ── ChaCha20-Poly1305 ─────────────────────────────────────────────────────
    #[test]
    fn test_chacha20poly1305_round_trip() {
        round_trip::<ChaCha20Poly1305>("chacha20-poly1305", &[0xABu8; 32]);
    }

    #[test]
    fn test_chacha20poly1305_wrong_key() {
        wrong_key_returns_err::<ChaCha20Poly1305>(
            "chacha20-poly1305",
            &[0xABu8; 32],
            &[0x00u8; 32],
        );
    }

    // ── XChaCha20-Poly1305 ────────────────────────────────────────────────────
    #[test]
    fn test_xchacha20poly1305_round_trip() {
        round_trip::<XChaCha20Poly1305>("xchacha20-poly1305", &[0xCDu8; 32]);
    }

    #[test]
    fn test_xchacha20poly1305_wrong_key() {
        wrong_key_returns_err::<XChaCha20Poly1305>(
            "xchacha20-poly1305",
            &[0xCDu8; 32],
            &[0x00u8; 32],
        );
    }
}