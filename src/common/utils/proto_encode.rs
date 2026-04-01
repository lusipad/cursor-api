use crate::{
    app::route::InfallibleJson,
    common::model::{ApiStatus, GenericError},
};
use alloc::borrow::Cow;

const SIZE_LIMIT_MSG: &str = "Message exceeds 4 MiB size limit";

#[derive(Debug)]
pub struct ExceedSizeLimit;

impl ExceedSizeLimit {
    #[inline]
    pub const fn message() -> &'static str { SIZE_LIMIT_MSG }

    #[inline]
    pub const fn into_generic(self) -> GenericError {
        GenericError {
            status: ApiStatus::Error,
            code: Some(http::StatusCode::PAYLOAD_TOO_LARGE),
            error: Some(Cow::Borrowed("resource_exhausted")),
            message: Some(Cow::Borrowed(SIZE_LIMIT_MSG)),
        }
    }

    #[inline]
    pub const fn into_response_tuple(self) -> (http::StatusCode, InfallibleJson<GenericError>) {
        (http::StatusCode::PAYLOAD_TOO_LARGE, InfallibleJson(self.into_generic()))
    }
}

impl core::fmt::Display for ExceedSizeLimit {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(SIZE_LIMIT_MSG)
    }
}

impl ::core::error::Error for ExceedSizeLimit {}

impl axum::response::IntoResponse for ExceedSizeLimit {
    #[inline]
    fn into_response(self) -> axum::response::Response {
        self.into_response_tuple().into_response()
    }
}

// ─── compression ────────────────────────────────────────────────────────────

const COMPRESSION_THRESHOLD: usize = 1024;

/// 尝试压缩，仅当压缩后体积更小时返回。
/// ≤ 1KB 直接跳过。
#[inline]
fn try_compress_if_beneficial(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() <= COMPRESSION_THRESHOLD {
        return None;
    }
    let compressed = grpc_stream::compress_gzip(data);
    if compressed.len() < data.len() { Some(compressed) } else { None }
}

// ─── encode_message (non-framed, with compression) ─────────────────────────

/// 编码 protobuf 消息，自动压缩优化。
///
/// **编码策略**：调用 `encoded_len` 一次用于预分配 + 大小校验，
/// 然后 `encode_raw` 一次。内部嵌套消息通过 prost backfill 补丁
/// 不再递归调用 `encoded_len`，总遍历次数 = 2（而非 O(D+1)）。
///
/// # Returns
/// `Ok((data, is_compressed))`
#[inline(always)]
pub fn encode_message(message: &impl ::prost::Message) -> Result<(Vec<u8>, bool), ExceedSizeLimit> {
    let estimated_size = message.encoded_len();

    if estimated_size > grpc_stream::MAX_DECOMPRESSED_SIZE_BYTES {
        __cold_path!();
        return Err(ExceedSizeLimit);
    }

    let mut encoded = Vec::with_capacity(estimated_size);
    message.encode_raw(&mut encoded);

    if let Some(compressed) = try_compress_if_beneficial(&encoded) {
        Ok((compressed, true))
    } else {
        Ok((encoded, false))
    }
}

// ─── frame helpers ──────────────────────────────────────────────────────────

/// gRPC 帧头固定 5 字节: `[compression_flag: u8][body_len: u32 BE]`
const FRAME_HEADER_LEN: usize = 5;

/// 将压缩标志 + 长度写入 `buf[pos..pos+5]`。
///
/// # Safety
/// `buf.len() >= pos + 5`
#[inline(always)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn write_frame_header(buf: &mut Vec<u8>, pos: usize, compressed: bool, body_len: usize) {
    let ptr = buf.as_mut_ptr().add(pos);
    *ptr = compressed as u8;
    let len_bytes = (body_len as u32).to_be_bytes();
    ::core::ptr::copy_nonoverlapping(len_bytes.as_ptr(), ptr.add(1), 4);
}

/// 对 `buf[body_start..body_end]` 尝试压缩。
/// 如果压缩有效，原地替换并截断 buf，返回 `(compressed, final_body_len)`。
/// 否则保持不变，返回 `(false, original_body_len)`。
#[inline]
fn compress_frame_body_in_place(
    buf: &mut Vec<u8>,
    body_start: usize,
    body_end: usize,
) -> (bool, usize) {
    let body = &buf[body_start..body_end];
    let original_len = body.len();

    if let Some(compressed) = try_compress_if_beneficial(body) {
        let compressed_len = compressed.len();
        // compressed_len < original_len 由 try_compress_if_beneficial 保证
        unsafe {
            ::core::ptr::copy_nonoverlapping(
                compressed.as_ptr(),
                buf.as_mut_ptr().add(body_start),
                compressed_len,
            );
        }
        buf.truncate(body_start + compressed_len);
        (true, compressed_len)
    } else {
        (false, original_len)
    }
}

// ─── encode_message_framed (single frame) ───────────────────────────────────

/// 编码单条 protobuf 消息为 gRPC 帧。
///
/// **单消息策略**：仍用 `encoded_len` 预分配（一次精确分配优于 Vec 增长），
/// 但内部嵌套通过 backfill 补丁只做一次编码遍历。
///
/// # 协议格式
/// ```text
/// [compression_flag 1B][body_len 4B BE][body]
/// ```
#[inline(always)]
pub fn encode_message_framed(message: &impl ::prost::Message) -> Result<Vec<u8>, ExceedSizeLimit> {
    let estimated_size = message.encoded_len();

    if estimated_size > grpc_stream::MAX_DECOMPRESSED_SIZE_BYTES {
        __cold_path!();
        return Err(ExceedSizeLimit);
    }

    // 128 字节余量覆盖 backfill 临时 varint placeholder 膨胀
    // (每层嵌套最多 +9 字节，128 覆盖 ~14 层)
    let mut buf = Vec::<u8>::with_capacity(FRAME_HEADER_LEN + estimated_size + 128);

    // 占位 5 字节头部（稍后回填）
    // Safety: u8 无 validity requirement，且 write_frame_header 在返回前覆盖
    unsafe { buf.set_len(FRAME_HEADER_LEN) };

    // 单次 encode_raw → 内部嵌套走 backfill，不再递归 encoded_len
    message.encode_raw(&mut buf);

    let body_end = buf.len();
    let body_start = FRAME_HEADER_LEN;

    // 校验实际编码长度（防御性，理论上 == estimated_size）
    let actual_body_len = body_end - body_start;
    if actual_body_len > grpc_stream::MAX_DECOMPRESSED_SIZE_BYTES {
        __cold_path!();
        return Err(ExceedSizeLimit);
    }

    // 压缩 + 回填头部
    let (compressed, final_len) = compress_frame_body_in_place(&mut buf, body_start, body_end);
    unsafe { write_frame_header(&mut buf, 0, compressed, final_len) };

    Ok(buf)
}

// ─── encode_messages_framed (multi-frame, zero redundant traversal) ─────────

/// 将多条 protobuf 消息编码为连续 gRPC 帧。
///
/// **极限优化**：完全跳过 `encoded_len`，每条消息只做一次 `encode_raw` 遍历。
/// 利用 prost backfill 补丁，嵌套消息也不会重复计算长度。
///
/// # 内部流程
/// ```text
/// for each message:
///   1. 记录 buf 当前偏移 frame_start
///   2. 占位 5 字节 header
///   3. encode_raw 直接追加到 buf（一次遍历）
///   4. body_len = buf.len() - frame_start - 5
///   5. 校验 body_len ≤ 4MiB
///   6. 尝试压缩 → 原地替换 + truncate
///   7. 回填 header
///   8. 用本帧实际大小为后续帧预留容量
/// ```
///
/// # 容量策略
/// - 首帧：无预知大小，Vec 自然增长（encode_raw 通过 BufMut push）
/// - 后续帧：`remaining * (first_frame_size + 16)` 一次 reserve
///   - +16 补偿压缩/编码波动
///   - 后续帧不再 realloc（除非消息大小差异极大）
///
/// # Errors
/// 任一消息编码后超 4MiB 立即返回错误，已编码的数据被丢弃。
#[inline(always)]
pub fn encode_messages_framed<M: ::prost::Message>(
    messages: &[M],
) -> Result<Vec<u8>, ExceedSizeLimit> {
    match messages.len() {
        0 => return Ok(Vec::new()),
        1 => return encode_message_framed(&messages[0]),
        _ => {}
    }

    let count = messages.len();
    let mut buf = Vec::<u8>::new();

    for (i, msg) in messages.iter().enumerate() {
        let frame_start = buf.len();

        // 占位 header
        buf.reserve(FRAME_HEADER_LEN);
        unsafe {
            buf.set_len(frame_start + FRAME_HEADER_LEN);
        }

        // 单次遍历编码（backfill 处理嵌套长度前缀）
        msg.encode_raw(&mut buf);

        let body_end = buf.len();
        let body_start = frame_start + FRAME_HEADER_LEN;
        let actual_body_len = body_end - body_start;

        // 大小校验
        if actual_body_len > grpc_stream::MAX_DECOMPRESSED_SIZE_BYTES {
            __cold_path!();
            return Err(ExceedSizeLimit);
        }

        // 压缩 + 回填
        let (compressed, final_len) = compress_frame_body_in_place(&mut buf, body_start, body_end);
        unsafe { write_frame_header(&mut buf, frame_start, compressed, final_len) };

        // 首帧完成后，为剩余帧一次性预留
        if i == 0 {
            let first_frame_size = buf.len(); // header + final_body
            let remaining = count - 1;
            buf.reserve(remaining * (first_frame_size + 16));
        }
    }

    Ok(buf)
}
