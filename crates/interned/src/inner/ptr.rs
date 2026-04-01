use super::repr::ArcStrInner;
use core::{
    hash::{Hash, Hasher},
    ptr::NonNull,
};
use scc::Equivalent;

/// 线程安全的 `ArcStrInner` 指针包装
///
/// 这是字符串池 [`PtrMap`](crate::PtrMap) 的键类型。
/// 封装了指向 `ArcStrInner` 的 `NonNull` 指针，并提供：
///
/// - 基于预存哈希值的 `Hash` 实现（配合 [`IdentityHasher`](crate::pool::IdentityHasher)）
/// - 基于指针地址的 `PartialEq` 实现（池不变量保证内容相同 ⟹ 地址相同）
/// - `Equivalent<ThreadSafePtr> for str`：支持用 `&str` 在池中查找
///
/// # Safety
///
/// 虽然包装了裸指针，但线程安全性由以下不变量保证：
/// - `ArcStrInner` 的字符串内容不可变
/// - 引用计数使用原子操作
/// - 指针生命周期由全局池管理
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct ThreadSafePtr(pub(crate) NonNull<ArcStrInner>);

unsafe impl Send for ThreadSafePtr {}
unsafe impl Sync for ThreadSafePtr {}

impl Hash for ThreadSafePtr {
    /// 直接透传 `ArcStrInner` 中预存的 ahash 哈希值
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) { unsafe { state.write_u64(self.0.as_ref().hash) } }
}

impl PartialEq for ThreadSafePtr {
    /// 指针地址比较——池保证相同内容不会有两个不同地址的条目
    #[inline]
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
}

impl Eq for ThreadSafePtr {}

impl Equivalent<ThreadSafePtr> for str {
    /// 用字符串内容在池中查找对应的 `ThreadSafePtr`
    ///
    /// 先比较长度（O(1)），长度相等时才比较内容（O(n)）。
    #[inline]
    fn equivalent(&self, key: &ThreadSafePtr) -> bool {
        unsafe {
            let inner = key.0.as_ref();
            inner.string_len == self.len() && inner.as_str() == self
        }
    }
}
