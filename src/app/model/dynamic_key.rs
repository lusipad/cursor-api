use super::token::RawToken;
use crate::{
    app::constant::{TYPE_SESSION, TYPE_WEB},
    common::utils::hex::hex_to_byte,
};
use alloc::sync::Arc;
use arc_swap::ArcSwap;
use hmac::{Hmac, KeyInit, Mac as _};
use manually_init::ManuallyInit;
use sha2::{
    Digest as _, Sha256,
    digest::{FixedOutput, array::Array},
};

pub struct Secret(pub Option<[u8; 64]>);

impl Secret {
    pub fn parse_str(s: &str) -> Self {
        let Some(s) = s.split_whitespace().next() else {
            return Secret(None);
        };

        let mut result = [0u8; 64];

        if let Some(hex_str) = s.strip_prefix("hex:")
            && hex_str.len() >= 64
        {
            let hex_bytes = hex_str.as_bytes();
            let cap = hex_bytes.len().min(128);
            let even = cap & !1; // 向下对齐偶数，因为奇数会被 ensure! 拒绝

            if let Ok(decoded) = hex_simd::decode(
                &hex_bytes[..even],
                hex_simd::Out::from_slice(&mut result), // 64 >= even/2，直接传整个数组
            ) {
                let n = decoded.len();
                if even < cap && n < 64 {
                    if let Some(b) = hex_to_byte(hex_bytes[even], b'0') {
                        result[n] = b;
                    }
                }
                return Secret(Some(result));
            }
        }

        let out: &mut [u8; 32] = unsafe { &mut *result.as_mut_ptr().cast() };
        FixedOutput::finalize_into(Sha256::new().chain_update(s), out.into());

        Secret(Some(result))
    }
}

static KEY: ManuallyInit<ArcSwap<Hmac<Sha256>>> = ManuallyInit::new();

pub fn init(secret: [u8; 64]) { KEY.init(ArcSwap::from_pointee(KeyInit::new(&Array(secret)))) }

pub fn update(secret: [u8; 64]) { KEY.store(Arc::new(KeyInit::new(&Array(secret)))) }

pub fn get_hash(raw: &RawToken) -> [u8; 32] {
    let mut hmac = KEY.get().load().as_ref().clone();
    hmac.update(b"subject");
    hmac.update(raw.subject.provider.as_str().as_bytes());
    hmac.update(raw.subject.id.as_bytes());
    hmac.update(b"signature");
    hmac.update(&raw.signature);
    hmac.update(b"duration");
    hmac.update(&raw.duration.start.to_ne_bytes());
    hmac.update(&raw.duration.end.to_ne_bytes());
    hmac.update(b"randomness");
    hmac.update(&raw.randomness.to_bytes());
    hmac.update(b"type");
    hmac.update(if raw.is_session { TYPE_SESSION } else { TYPE_WEB }.as_bytes());
    if !raw.workos_session_id.is_empty() {
        hmac.update(b"workos\x1fsession\x1fid");
        hmac.update(raw.workos_session_id.as_bytes());
    }
    hmac.finalize_fixed().0
}
