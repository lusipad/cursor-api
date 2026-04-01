mod hasher;

use core::alloc::Layout;
pub use hasher::{IdentityHasher, PoolHasher, PtrMap};
use manually_init::ManuallyInit;

/// 字符串池抽象——定义池化字符串的存储与分配策略
///
/// 通过实现此 trait，可以将 `ArcStr` 绑定到不同的池后端
/// （例如独立的测试池、自定义分配器等）。
///
/// 默认实现 [`GlobalPool`] 使用全局静态 `HashMap` + 系统分配器。
///
/// # 必须实现
///
/// - [`get_pool`](StringPool::get_pool)：返回池的并发 `HashMap` 引用
///
/// # 可选覆盖
///
/// - [`allocate`](StringPool::allocate) / [`deallocate`](StringPool::deallocate)：
///   替换底层内存分配器（默认委托 `alloc::alloc`）
///
/// # Safety Contract
///
/// 实现者必须保证 `get_pool()` 返回的引用在整个程序生命周期内有效。
pub trait StringPool: 'static + Send + Sync {
    /// 获取该池对应的全局并发 [`PtrMap`] 实例
    fn get_pool() -> &'static PtrMap;

    /// 分配 `ArcStrInner` 所需的内存块
    ///
    /// 默认使用 `alloc::alloc::alloc`。自定义分配器可覆盖此方法。
    ///
    /// # Safety
    ///
    /// - `layout` 必须由 `ArcStrInner::layout_for_string` 生成
    /// - 返回的指针必须满足 `layout` 的大小和对齐要求
    /// - 返回 null 表示分配失败（调用方会触发 `handle_alloc_error`）
    #[inline]
    unsafe fn allocate(layout: Layout) -> *mut u8 { alloc::alloc::alloc(layout) }

    /// 释放由 [`allocate`](StringPool::allocate) 分配的内存块
    ///
    /// # Safety
    ///
    /// - `ptr` 必须由同一池的 `allocate` 返回
    /// - `layout` 必须与分配时使用的布局完全一致
    /// - 调用后 `ptr` 不再有效
    #[inline]
    unsafe fn deallocate(ptr: *mut u8, layout: Layout) { alloc::alloc::dealloc(ptr, layout) }
}

pub(crate) static ARC_STR_POOL: ManuallyInit<PtrMap> = ManuallyInit::new();

/// 默认的全局字符串池
///
/// 使用进程级静态 [`PtrMap`] 存储所有池化字符串，
/// 配合系统分配器进行内存管理。这是 [`ArcStr`](crate::ArcStr) 的默认池类型。
///
/// # Examples
///
/// ```rust
/// use interned::{ArcStr, pool::GlobalPool};
///
/// // 以下两者等价：
/// let s1 = ArcStr::new("hello");
/// let s2 = ArcStr::<GlobalPool>::new("hello");
/// assert_eq!(s1, s2);
/// ```
pub struct GlobalPool;

impl StringPool for GlobalPool {
    #[inline]
    fn get_pool() -> &'static PtrMap { ARC_STR_POOL.get() }
}
