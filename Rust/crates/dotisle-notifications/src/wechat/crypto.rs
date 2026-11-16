// SQLCipher 4 page codec. Key material is never serialized or logged.
use aes::{
    cipher::{generic_array::GenericArray, BlockDecrypt, KeyInit},
    Aes256,
};
use sha2::{Digest, Sha512};
pub const PAGE: usize = 4096;
pub const END: usize = 4016;
#[derive(Clone)]
pub struct Key {
    enc: [u8; 32],
    mac: [u8; 32],
}
impl Drop for Key {
    fn drop(&mut self) {
        for byte in self.enc.iter_mut().chain(self.mac.iter_mut()) {
            unsafe { std::ptr::write_volatile(byte, 0) }
        }
    }
}
fn hmac(key: &[u8], parts: &[&[u8]]) -> [u8; 64] {
    let mut pad = [0x36; 128];
    for (p, k) in pad.iter_mut().zip(key) {
        *p ^= k
    }
    let mut h = Sha512::new();
    h.update(pad);
    for part in parts {
        h.update(part)
    }
    let inner = h.finalize();
    for p in &mut pad {
        *p ^= 0x36 ^ 0x5c
    }
    let mut h = Sha512::new();
    h.update(pad);
    h.update(inner);
    h.finalize().into()
}
impl Key {
    pub fn verified(enc: [u8; 32], salt: &[u8], page: &[u8]) -> Option<Self> {
        if salt.len() != 16 || page.len() != PAGE {
            return None;
        }
        let salt = salt.iter().map(|b| b ^ 0x3a).collect::<Vec<_>>();
        let a = hmac(&enc, &[&salt, &1u32.to_be_bytes()]);
        let b = hmac(&enc, &[&a]);
        let mut mac = [0; 32];
        for i in 0..32 {
            mac[i] = a[i] ^ b[i]
        }
        let k = Self { enc, mac };
        k.check(page, 1).then_some(k)
    }
    fn check(&self, page: &[u8], pgno: u32) -> bool {
        if page.len() != PAGE {
            return false;
        }
        let from = if pgno == 1 { 16 } else { 0 };
        let mac = hmac(&self.mac, &[&page[from..4032], &pgno.to_le_bytes()]);
        mac.iter()
            .zip(&page[4032..])
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
    }
    pub fn decrypt(&self, page: &mut [u8; PAGE], pgno: u32) -> Result<(), String> {
        if !self.check(page, pgno) {
            return Err("微信数据页校验失败，稍后自动重试".into());
        }
        let cipher = Aes256::new(GenericArray::from_slice(&self.enc));
        let mut previous = [0; 16];
        previous.copy_from_slice(&page[END..END + 16]);
        let from = if pgno == 1 { 16 } else { 0 };
        for block in page[from..END].chunks_exact_mut(16) {
            let mut saved = [0; 16];
            saved.copy_from_slice(block);
            cipher.decrypt_block(GenericArray::from_mut_slice(block));
            for i in 0..16 {
                block[i] ^= previous[i]
            }
            previous = saved;
        }
        page[END..].fill(0);
        if pgno == 1 {
            page[..16].copy_from_slice(b"SQLite format 3\0");
            page[18] = 1;
            page[19] = 1;
        }
        Ok(())
    }
}
#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use aes::cipher::BlockEncrypt;
    pub fn seal(enc: [u8; 32], salt: [u8; 16], plain: &[u8; PAGE], pgno: u32) -> [u8; PAGE] {
        let mut page = *plain;
        let iv = [0x61; 16];
        let mut previous = iv;
        let cipher = Aes256::new(GenericArray::from_slice(&enc));
        for block in page[if pgno == 1 { 16 } else { 0 }..END].chunks_exact_mut(16) {
            for i in 0..16 {
                block[i] ^= previous[i]
            }
            cipher.encrypt_block(GenericArray::from_mut_slice(block));
            previous.copy_from_slice(block);
        }
        if pgno == 1 {
            page[..16].copy_from_slice(&salt)
        }
        page[END..END + 16].copy_from_slice(&iv);
        let ms = salt.map(|x| x ^ 0x3a);
        let a = hmac(&enc, &[&ms, &1u32.to_be_bytes()]);
        let b = hmac(&enc, &[&a]);
        let mut mk = [0; 32];
        for i in 0..32 {
            mk[i] = a[i] ^ b[i]
        }
        let m = hmac(
            &mk,
            &[
                &page[if pgno == 1 { 16 } else { 0 }..4032],
                &pgno.to_le_bytes(),
            ],
        );
        page[4032..].copy_from_slice(&m);
        page
    }
    #[test]
    fn authenticated_page_roundtrip_and_tamper() {
        let enc = [7; 32];
        let salt = [3; 16];
        let mut p = [42; PAGE];
        p[..16].copy_from_slice(b"SQLite format 3\0");
        p[18] = 1;
        p[19] = 1;
        let mut c = seal(enc, salt, &p, 1);
        let k = Key::verified(enc, &salt, &c).unwrap();
        assert!(Key::verified([8; 32], &salt, &c).is_none());
        k.decrypt(&mut c, 1).unwrap();
        assert_eq!(&c[..END], &p[..END]);
        let mut c = seal(enc, salt, &p, 2);
        c[70] ^= 1;
        assert!(k.decrypt(&mut c, 2).is_err());
    }
}
