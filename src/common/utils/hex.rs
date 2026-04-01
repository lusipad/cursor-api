//! 十六进制编解码工具

/// 解码查找表：将ASCII字符映射到0-15或0xFF（非法）
pub(crate) const HEX_TABLE: &[u8; 256] = &{
    let mut buf = [0xFF; 256]; // 默认非法值
    let mut i: u8 = 0;
    loop {
        buf[i as usize] = match i {
            b'0'..=b'9' => i - b'0',
            b'a'..=b'f' => i - b'a' + 10,
            b'A'..=b'F' => i - b'A' + 10,
            _ => 0xFF,
        };
        if i == 255 {
            break buf;
        }
        i += 1;
    }
};

/// 解码两个十六进制字符为一个字节
#[inline(always)]
pub const fn hex_to_byte(hi: u8, lo: u8) -> Option<u8> {
    let high = HEX_TABLE[hi as usize];
    if high == 0xFF {
        return None;
    }
    let low = HEX_TABLE[lo as usize];
    if low == 0xFF {
        return None;
    }
    Some((high << 4) | low) // 直接位移，无需查表
}

pub(crate) fn encode(data: &[u8]) -> String {
    hex_simd::encode_to_string(data, hex_simd::AsciiCase::Lower)
}

#[allow(unused)]
pub(crate) fn decode(data: &str) -> Result<Vec<u8>, hex_simd::Error> {
    hex_simd::decode_to_vec(data)
}
