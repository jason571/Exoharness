use sha2::{Digest, Sha256};
/// Identity is the pair (provider, raw name), never reparsed from display text.
pub fn public_name(provider: &str, raw: &str) -> String {
    let digest = format!("{:x}", Sha256::digest(format!("{provider}\0{raw}")));
    let label: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    format!("mcp_{label}_{}", &digest[..16])
}
