use super::super::inner::ThreadSafePtr;
use core::hash::{BuildHasherDefault, Hasher};
use scc::HashMap;

/// 透传哈希器——直接使用 `ArcStrInner` 中预存的哈希值
///
/// 字符串池内部的 `HashMap` 使用此哈希器，避免对已知哈希值
/// 进行二次计算。工作流程：
///
/// 1. [`ThreadSafePtr::hash()`] 调用 `write_u64(stored_hash)`
/// 2. `IdentityHasher` 直接存储该值
/// 3. `finish()` 原样返回
///
/// # Panics
///
/// 调用 `write()` 会 panic——此哈希器仅支持 `write_u64`。
#[derive(Default, Clone, Copy)]
pub struct IdentityHasher(u64);

impl Hasher for IdentityHasher {
    fn write(&mut self, _: &[u8]) {
        unreachable!("IdentityHasher: only write_u64 is supported");
    }

    #[inline]
    fn write_u64(&mut self, id: u64) { self.0 = id; }

    #[inline]
    fn finish(&self) -> u64 { self.0 }
}

/// 池内部 `HashMap` 使用的哈希构建器
///
/// 基于 [`IdentityHasher`]，将 `BuildHasher::build_hasher()` 的返回值
/// 绑定为透传哈希器。
pub type PoolHasher = BuildHasherDefault<IdentityHasher>;

/// 字符串池的并发 `HashMap` 类型
///
/// - **键**：[`ThreadSafePtr`] — 指向 `ArcStrInner` 的指针包装
/// - **值**：`()` — 仅需要键的存在性
/// - **哈希器**：[`PoolHasher`] — 透传预计算哈希值
///
/// 使用 [`scc::HashMap`] 提供无锁并发读写。
pub type PtrMap = HashMap<ThreadSafePtr, (), PoolHasher>;
