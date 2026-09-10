//! 登录凭据加密：与工作流模板中的 openssl 解密命令逐字节配对（ADR-0002）。
//!
//! 加密格式与 `openssl enc -aes-256-cbc -pbkdf2 -a -A -pass "pass:KEY"` 完全一致：
//! `Salted__` magic + 8 字节随机盐 + AES-256-CBC（PKCS7 填充），
//! 密钥与 IV 由 PBKDF2-HMAC-SHA256（10000 轮）派生 48 字节（key 32 + IV 16），
//! 输出为单行标准 base64。工作流侧用
//! `openssl enc -d -aes-256-cbc -pbkdf2 -a -A -pass "pass:$AUTH_PASSPHRASE"` 解密。

use aes::cipher::{BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sha2::Sha256;

/// PBKDF2 迭代轮数（openssl `-pbkdf2` 默认值）。
const PBKDF2_ROUNDS: u32 = 10_000;
/// 派生长度：AES-256 密钥 32 字节 + CBC IV 16 字节。
const DERIVED_LEN: usize = 48;
const KEY_LEN: usize = 32;
const SALT_LEN: usize = 8;
const MAGIC: &[u8; 8] = b"Salted__";

type Block = aes::Block;
type Cipher = aes::Aes256;
const BLOCK_SIZE: usize = 16;

fn derive_key_iv(password: &[u8], salt: &[u8]) -> [u8; DERIVED_LEN] {
    let mut dk = [0u8; DERIVED_LEN];
    pbkdf2::pbkdf2_hmac::<Sha256>(password, salt, PBKDF2_ROUNDS, &mut dk);
    dk
}

/// PKCS7 填充（块大小 16）。
fn pkcs7_pad(data: &[u8]) -> Vec<u8> {
    let pad = (BLOCK_SIZE - data.len() % BLOCK_SIZE) as u8;
    let mut out = data.to_vec();
    out.extend(std::iter::repeat_n(pad, pad as usize));
    out
}

/// 去除 PKCS7 填充（填充非法时报错）。
fn pkcs7_unpad(data: &[u8]) -> Result<&[u8], String> {
    if data.is_empty() || !data.len().is_multiple_of(BLOCK_SIZE) {
        return Err(crate::i18n::err_decrypt("invalid ciphertext length").to_string());
    }
    let pad = data[data.len() - 1] as usize;
    if pad == 0 || pad > BLOCK_SIZE || data[data.len() - pad..] != [pad as u8; BLOCK_SIZE][..pad] {
        return Err(crate::i18n::err_decrypt("bad decrypt").to_string());
    }
    Ok(&data[..data.len() - pad])
}

/// CBC 加密一段已 PKCS7 填充的数据（密文就地写入 `ct` 对应块）。
fn cbc_encrypt(cipher: &Cipher, iv: &[u8], padded: &[u8], ct: &mut Vec<u8>) {
    let mut prev: [u8; BLOCK_SIZE] = iv.try_into().expect("IV 长度固定 16");
    for chunk in padded.as_chunks::<BLOCK_SIZE>().0 {
        let mut block = Block::try_from(chunk.as_slice()).expect("块长度固定 16");
        for (b, p) in block.iter_mut().zip(&prev) {
            *b ^= p;
        }
        cipher.encrypt_block(&mut block);
        prev.copy_from_slice(block.as_ref());
        ct.extend_from_slice(block.as_ref());
    }
}

/// CBC 解密整段密文（返回含填充的明文）。
fn cbc_decrypt(cipher: &Cipher, iv: &[u8], ct: &[u8]) -> Result<Vec<u8>, String> {
    let mut prev: [u8; BLOCK_SIZE] = iv.try_into().expect("IV 长度固定 16");
    let mut pt = Vec::with_capacity(ct.len());
    for chunk in ct.as_chunks::<BLOCK_SIZE>().0 {
        let mut block = Block::try_from(chunk.as_slice()).expect("块长度固定 16");
        let ciphertext = *block.as_ref();
        cipher.decrypt_block(&mut block);
        for (b, p) in block.iter_mut().zip(&prev) {
            *b ^= p;
        }
        prev = ciphertext;
        pt.extend_from_slice(block.as_ref());
    }
    Ok(pt)
}

/// 把 auth（base64 的 `用户名:密码`）用加密口令加密成 `target_auth_secret`（单行 base64）。
pub fn encrypt_auth(auth: &str, auth_passphrase: &str) -> Result<String, String> {
    if auth_passphrase.is_empty() {
        return Err(crate::i18n::err_missing_auth_passphrase().to_string());
    }
    let mut salt = [0u8; SALT_LEN];
    getrandom::fill(&mut salt).map_err(crate::i18n::err_encrypt)?;

    let dk = derive_key_iv(auth_passphrase.as_bytes(), &salt);
    let (key, iv) = dk.split_at(KEY_LEN);
    let cipher = Cipher::new(key.try_into().expect("密钥长度固定 32"));
    let iv: [u8; BLOCK_SIZE] = iv.try_into().expect("IV 长度固定 16");

    let padded = pkcs7_pad(auth.as_bytes());
    let mut ct = Vec::with_capacity(padded.len());
    cbc_encrypt(&cipher, &iv, &padded, &mut ct);

    let mut blob = Vec::with_capacity(8 + SALT_LEN + ct.len());
    blob.extend_from_slice(MAGIC);
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&ct);
    Ok(STANDARD.encode(blob))
}

/// 解密 `target_auth_secret`，还原 auth（base64 的 `用户名:密码`）。
/// 与工作流模板的 openssl 解密命令配对，供本地校验与测试使用。
pub fn decrypt_auth(secret_b64: &str, auth_passphrase: &str) -> Result<String, String> {
    if auth_passphrase.is_empty() {
        return Err(crate::i18n::err_missing_auth_passphrase().to_string());
    }
    let blob = STANDARD
        .decode(secret_b64.trim())
        .map_err(|e| crate::i18n::err_decrypt(&e))?;
    if blob.len() < 16 || &blob[..8] != MAGIC {
        return Err(crate::i18n::err_decrypt("bad magic number").to_string());
    }
    let (salt, ct) = blob[8..].split_at(SALT_LEN);
    if ct.is_empty() || ct.len() % BLOCK_SIZE != 0 {
        return Err(crate::i18n::err_decrypt("invalid ciphertext length").to_string());
    }
    let dk = derive_key_iv(auth_passphrase.as_bytes(), salt);
    let (key, iv) = dk.split_at(KEY_LEN);
    let cipher = Cipher::new(key.try_into().expect("密钥长度固定 32"));
    let iv: [u8; BLOCK_SIZE] = iv.try_into().expect("IV 长度固定 16");
    let pt = cbc_decrypt(&cipher, &iv, ct)?;
    let plain = pkcs7_unpad(&pt)?;
    String::from_utf8(plain.to_vec()).map_err(|e| crate::i18n::err_decrypt(&e))
}

/// 生成随机加密口令（32 字节 CSPRNG 熵，base64 编码为 44 个可打印字符）。
/// 与 `openssl rand -base64 24` 同等强度，可直接作为 `[setting].auth_passphrase`
/// 与 GitHub Secret `AUTH_PASSPHRASE` 的值。
pub fn generate_passphrase() -> Result<String, String> {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).map_err(crate::i18n::err_encrypt)?;
    Ok(STANDARD.encode(buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "mtrans-test-key";
    /// openssl 生成的参考向量（随机盐）：
    /// `printf 'YTox' | openssl enc -aes-256-cbc -pbkdf2 -a -A -pass "pass:mtrans-test-key"`
    const OPENSSL_VECTOR: &str = "U2FsdGVkX19lnxIk/mAMysc6PM61Q9C8xZjQ4DAw6w0=";
    const PLAIN_AUTH: &str = "YTox";

    #[test]
    fn decrypt_openssl_vector() {
        assert_eq!(decrypt_auth(OPENSSL_VECTOR, KEY).unwrap(), PLAIN_AUTH);
    }

    #[test]
    fn round_trip() {
        let secret = encrypt_auth(PLAIN_AUTH, KEY).unwrap();
        assert_eq!(decrypt_auth(&secret, KEY).unwrap(), PLAIN_AUTH);
    }

    #[test]
    fn encrypt_is_salted_and_randomized() {
        let a = encrypt_auth(PLAIN_AUTH, KEY).unwrap();
        let b = encrypt_auth(PLAIN_AUTH, KEY).unwrap();
        assert_ne!(a, b, "随机盐下同一明文两次加密结果不同");
        let blob = STANDARD.decode(&a).unwrap();
        assert_eq!(&blob[..8], MAGIC);
        assert_eq!(decrypt_auth(&a, KEY).unwrap(), PLAIN_AUTH);
    }

    #[test]
    fn wrong_key_fails() {
        let secret = encrypt_auth(PLAIN_AUTH, KEY).unwrap();
        assert!(decrypt_auth(&secret, "other-key").is_err());
    }

    #[test]
    fn empty_key_rejected() {
        assert!(encrypt_auth(PLAIN_AUTH, "").is_err());
        assert!(decrypt_auth(OPENSSL_VECTOR, "").is_err());
    }

    /// 本机存在 openssl 时，验证加密输出能被工作流模板中的解密命令解开（真实配对）。
    #[test]
    fn encrypt_pairs_with_openssl_decrypt() {
        let Ok(out) = std::process::Command::new("openssl")
            .arg("version")
            .output()
        else {
            return;
        };
        if !out.status.success() {
            return;
        }
        let secret = encrypt_auth(PLAIN_AUTH, KEY).unwrap();
        use std::io::Write as _;
        let mut child = std::process::Command::new("openssl")
            .args([
                "enc",
                "-d",
                "-aes-256-cbc",
                "-pbkdf2",
                "-a",
                "-A",
                "-pass",
                &format!("pass:{KEY}"),
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("启动 openssl 失败");
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(secret.as_bytes())
            .expect("写入 stdin 失败");
        let out = child.wait_with_output().expect("等待 openssl 失败");
        assert!(
            out.status.success(),
            "openssl 解密失败: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&out.stdout), PLAIN_AUTH);
    }
}
