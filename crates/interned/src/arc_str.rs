mod convert;
mod lifecycle;
mod traits;

#[cfg(feature = "rkyv")]
mod rkyv;
#[cfg(feature = "serde")]
mod serde;

use crate::{
    inner::{ArcStrInner, ThreadSafePtr},
    pool::{GlobalPool, PtrMap, StringPool},
};
use core::{hint, marker::PhantomData, ptr::NonNull};
use manually_init::ManuallyInit;

/// 内容哈希计算器（ahash 随机状态）
pub(crate) static CONTENT_HASHER: ManuallyInit<ahash::RandomState> = ManuallyInit::new();

// ────────────────────────────────────────────────────────────────────────────
// Core type
// ────────────────────────────────────────────────────────────────────────────

/// 引用计数的不可变字符串，支持全局字符串池复用
///
/// 相同内容的字符串共享同一份堆内存。`clone()` 仅原子递增引用计数，
/// 最后一个引用释放时自动从池中移除并释放底层内存。
///
/// # 性能特征
///
/// | 操作 | 复杂度 | 说明 |
/// |------|--------|------|
/// | `new()` 首次 | O(1) + 池插入 | 堆分配 + HashMap 插入 |
/// | `new()` 命中 | O(1) | HashMap 查找 + 原子递增 |
/// | `clone()` | O(1) | 仅原子递增 |
/// | `drop()` | O(1) | 原子递减 + 可能的池清理 |
/// | `as_str()` | O(1) | 直接内存访问 |
/// | `==` | O(1) | 指针比较（池保证去重） |
///
/// # Examples
///
/// ```rust
/// use interned::ArcStr;
///
/// let s1 = ArcStr::new("hello");
/// let s2 = ArcStr::new("hello");
/// assert_eq!(s1.as_ptr(), s2.as_ptr()); // 共享内存
/// assert_eq!(s1.ref_count(), 2);
/// ```
#[repr(transparent)]
pub struct ArcStr<P: StringPool = GlobalPool> {
    pub(crate) ptr: NonNull<ArcStrInner>,
    _marker: PhantomData<(ArcStrInner, P)>,
}

unsafe impl<P: StringPool> Send for ArcStr<P> {}
unsafe impl<P: StringPool> Sync for ArcStr<P> {}

// ────────────────────────────────────────────────────────────────────────────
// Construction & access
// ────────────────────────────────────────────────────────────────────────────

impl<P: StringPool> ArcStr<P> {
    /// 创建或复用字符串实例
    ///
    /// 先在池中查找（读路径），未命中时创建并插入（写路径），
    /// 写路径内含双重检查以避免并发重复创建。
    ///
    /// # 性能
    ///
    /// - **池命中**：O(1) HashMap 查找 + 原子递增
    /// - **池缺失**：O(1) 堆分配 + O(1) HashMap 插入
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::ArcStr;
    ///
    /// let s1 = ArcStr::new("shared");
    /// let s2 = ArcStr::new("shared"); // 复用 s1 的内存
    /// assert_eq!(s1.as_ptr(), s2.as_ptr());
    /// ```
    pub fn new<S: AsRef<str>>(s: S) -> Self {
        let string = s.as_ref();
        let hash = CONTENT_HASHER.hash_one(string);

        // 快速路径：池中已存在
        {
            let pool = P::get_pool();
            if let Some(existing) = Self::try_find_existing(pool, hash, string) {
                return existing;
            }
        }

        // 慢速路径：需要插入
        let pool = P::get_pool();

        match pool.raw_entry().from_key_hashed_nocheck_sync(hash, string) {
            scc::hash_map::RawEntry::Occupied(entry) => {
                let ptr = entry.key().0;
                unsafe { ptr.as_ref().inc_strong() };
                Self { ptr, _marker: PhantomData }
            }
            scc::hash_map::RawEntry::Vacant(entry) => {
                let layout = ArcStrInner::layout_for_string(string.len());

                let ptr = unsafe {
                    let alloc: *mut ArcStrInner = P::allocate(layout).cast();
                    if alloc.is_null() {
                        hint::cold_path();
                        alloc::alloc::handle_alloc_error(layout);
                    }
                    let ptr = NonNull::new_unchecked(alloc);
                    ArcStrInner::write_with_string(ptr, string, hash);
                    ptr
                };

                entry.insert_hashed_nocheck(hash, ThreadSafePtr(ptr), ());
                Self { ptr, _marker: PhantomData }
            }
        }
    }

    /// 获取字符串切片（零成本）
    ///
    /// 直接访问底层 UTF-8 数据，无间接寻址、无拷贝。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::ArcStr;
    ///
    /// let s = ArcStr::new("hello");
    /// assert_eq!(s.as_str(), "hello");
    /// ```
    #[must_use]
    #[inline]
    pub const fn as_str(&self) -> &str { unsafe { self.ptr.as_ref().as_str() } }

    /// 获取底层字节切片
    ///
    /// 返回字符串的 UTF-8 编码字节，与 [`str::as_bytes`] 语义一致。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::ArcStr;
    ///
    /// let s = ArcStr::new("hello");
    /// assert_eq!(s.as_bytes(), b"hello");
    /// ```
    #[must_use]
    #[inline]
    pub const fn as_bytes(&self) -> &[u8] { unsafe { self.ptr.as_ref().as_bytes() } }

    /// 字符串长度（UTF-8 字节数）
    ///
    /// 直接读取 `ArcStrInner::string_len` 字段，无需构造 `&str`。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::ArcStr;
    ///
    /// let s = ArcStr::new("hello");
    /// assert_eq!(s.len(), 5);
    /// ```
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize { unsafe { self.ptr.as_ref().string_len } }

    /// 是否为空字符串
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::ArcStr;
    ///
    /// assert!(ArcStr::new("").is_empty());
    /// assert!(!ArcStr::new("x").is_empty());
    /// ```
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool { self.len() == 0 }

    /// 当前引用计数的快照
    ///
    /// 由于并发访问，返回值可能在读取后立即过时。
    /// 主要用于调试和测试。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::ArcStr;
    ///
    /// let s1 = ArcStr::new("rc");
    /// assert_eq!(s1.ref_count(), 1);
    ///
    /// let s2 = s1.clone();
    /// assert_eq!(s1.ref_count(), 2);
    /// ```
    #[must_use]
    #[inline]
    pub fn ref_count(&self) -> usize { unsafe { self.ptr.as_ref().strong_count() } }

    /// 字符串数据的裸指针
    ///
    /// 指向 UTF-8 内容的首字节。可用于验证两个 `ArcStr`
    /// 是否共享同一底层内存。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::ArcStr;
    ///
    /// let s1 = ArcStr::new("ptr");
    /// let s2 = ArcStr::new("ptr");
    /// assert_eq!(s1.as_ptr(), s2.as_ptr()); // 池化去重
    /// ```
    #[must_use]
    #[inline]
    pub const fn as_ptr(&self) -> *const u8 { unsafe { self.ptr.as_ref().string_ptr() } }

    /// 在池中查找已存在的字符串并增加引用计数
    ///
    /// 使用预计算哈希 + `Equivalent` trait 进行 O(1) 查找，
    /// 避免重复哈希计算。
    #[must_use]
    #[inline]
    fn try_find_existing(pool: &PtrMap, hash: u64, string: &str) -> Option<Self> {
        use scc::hash_map::RawEntry;

        let RawEntry::Occupied(entry) = pool.raw_entry().from_key_hashed_nocheck_sync(hash, string)
        else {
            return None;
        };

        let ptr = entry.key().0;
        unsafe { ptr.as_ref().inc_strong() };
        Some(Self { ptr, _marker: PhantomData })
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Initialization
// ────────────────────────────────────────────────────────────────────────────

#[inline(always)]
#[allow(clippy::inline_always)]
pub(crate) fn __init() {
    CONTENT_HASHER.init(ahash::RandomState::new());
    crate::pool::ARC_STR_POOL.init(PtrMap::with_capacity_and_hasher(128, Default::default()));
}

// ────────────────────────────────────────────────────────────────────────────
// Test utilities
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
pub(crate) fn pool_stats() -> (usize, usize) {
    let pool = crate::pool::ARC_STR_POOL.get();
    (pool.len(), pool.capacity())
}

#[cfg(test)]
pub(crate) fn clear_pool_for_test() {
    std::thread::sleep(std::time::Duration::from_millis(10));
    crate::pool::ARC_STR_POOL.get().clear_sync();
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};

    fn run_isolated<F: FnOnce()>(f: F) {
        clear_pool_for_test();
        f();
        clear_pool_for_test();
    }

    #[test]
    fn basic_functionality() {
        run_isolated(|| {
            let s1 = ArcStr::new("hello");
            let s2 = ArcStr::new("hello");
            let s3 = ArcStr::new("world");

            assert_eq!(s1, s2);
            assert_ne!(s1, s3);
            assert_eq!(s1.ptr, s2.ptr);
            assert_ne!(s1.ptr, s3.ptr);

            assert_eq!(s1.as_str(), "hello");
            assert_eq!(s1.len(), 5);
            assert!(!s1.is_empty());

            let (count, _) = pool_stats();
            assert_eq!(count, 2);
        });
    }

    #[test]
    fn reference_counting() {
        run_isolated(|| {
            let s1 = ArcStr::<GlobalPool>::new("test");
            assert_eq!(s1.ref_count(), 1);

            let s2 = s1.clone();
            assert_eq!(s1.ref_count(), 2);
            assert_eq!(s1.ptr, s2.ptr);

            drop(s2);
            assert_eq!(s1.ref_count(), 1);

            drop(s1);
            thread::sleep(Duration::from_millis(5));
            assert_eq!(pool_stats().0, 0);
        });
    }

    #[test]
    fn pool_reuse() {
        run_isolated(|| {
            let s1 = ArcStr::<GlobalPool>::new("reuse_test");
            let s2 = ArcStr::<GlobalPool>::new("reuse_test");

            assert_eq!(s1.ptr, s2.ptr);
            assert_eq!(s1.ref_count(), 2);
            assert_eq!(pool_stats().0, 1);
        });
    }

    #[test]
    fn thread_safety() {
        run_isolated(|| {
            let s = ArcStr::new("shared");
            let handles: Vec<_> = (0..10)
                .map(|_| {
                    let s_clone = ArcStr::clone(&s);
                    thread::spawn(move || {
                        let local = ArcStr::new("shared");
                        assert_eq!(*s_clone, local);
                        assert_eq!(s_clone.ptr, local.ptr);
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    }

    #[test]
    fn empty_string() {
        run_isolated(|| {
            let empty = ArcStr::<GlobalPool>::new("");
            assert!(empty.is_empty());
            assert_eq!(empty.len(), 0);
            assert_eq!(empty.as_str(), "");
        });
    }

    #[test]
    fn from_implementations() {
        run_isolated(|| {
            use alloc::borrow::Cow;

            let s1 = ArcStr::from("from_str");
            let s2 = ArcStr::from(String::from("from_string"));
            let s3 = ArcStr::from(Cow::Borrowed("from_cow"));

            assert_eq!(s1.as_str(), "from_str");
            assert_eq!(s2.as_str(), "from_string");
            assert_eq!(s3.as_str(), "from_cow");
        });
    }
}
