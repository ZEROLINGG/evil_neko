
pub async fn tlsh_hash<T: AsRef<[u8]>>(data: T) -> Option<tlsh2::Tlsh128_1> {
    let mut builder = tlsh2::TlshDefaultBuilder::new();
    builder.update(data.as_ref());
    builder.build()
}

pub fn tlsh_to_string(hash: tlsh2::Tlsh128_1) -> String {
    String::from_utf8_lossy(&hash.hash()).into_owned()
}