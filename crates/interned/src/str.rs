//! 组合字符串类型——统一编译期字面量与运行时池化字符串
//!
//! `Str` 是面向用户的首选类型：大部分场景下用它替代 `&'static str`
//! 和 `ArcStr`，编译器常量用 `Static`，动态内容用 `Counted`，
//! API 层面完全统一。

mod convert;
mod traits;

#[cfg(feature = "rkyv")]
mod rkyv;
#[cfg(feature = "serde")]
mod serde;

use crate::arc_str::ArcStr;

// ────────────────────────────────────────────────────────────────────────────
// Core type
// ────────────────────────────────────────────────────────────────────────────

/// 组合字符串——编译期字面量或运行时引用计数字符串
///
/// # 选择指南
///
/// | 场景 | 推荐 | 原因 |
/// |------|------|------|
/// | 编译期已知的常量 | `Str::from_static("...")` | 零成本，永不释放 |
/// | 运行时动态内容 | `Str::new(s)` | 自动池化去重 |
/// | 需要 `const` 上下文 | `Str::from_static` | 是 `const fn` |
///
/// # 内存布局
///
/// ```text
/// size = 16, align = 8, needs_drop = true
///
///   Static(&'static str)   : fat pointer (ptr + len)     = 16 bytes
///   Counted(ArcStr)         : thin pointer (NonNull)      =  8 bytes
///
/// 编译器利用 niche 优化将判别值编码进高位，
/// 整个 enum 压缩到与 &str 相同的 16 字节。
/// ```
///
/// # Examples
///
/// ```rust
/// use interned::Str;
///
/// // 编译期常量
/// const GREETING: Str = Str::from_static("Hello");
///
/// // 运行时去重
/// let s1 = Str::new("world");
/// let s2 = Str::new("world");
/// assert_eq!(s1, s2);
/// ```
#[derive(Clone)]
pub enum Str {
    /// 编译期字面量——零分配、零释放、Clone 即指针拷贝
    Static(&'static str),
    /// 运行时池化字符串——原子引用计数，自动去重
    Counted(ArcStr),
}

// 编译期断言：确保 niche 优化生效
const _: () = assert!(core::mem::size_of::<Str>() == 16);
const _: () = assert!(core::mem::align_of::<Str>() == 8);

unsafe impl Send for Str {}
unsafe impl Sync for Str {}

// ────────────────────────────────────────────────────────────────────────────
// Construction & variant inspection
// ────────────────────────────────────────────────────────────────────────────

impl Str {
    /// 包装编译期字面量（`const fn`，零成本）
    ///
    /// 创建 `Static` 变体。不进入字符串池，不分配堆内存。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// const GREETING: Str = Str::from_static("Hello");
    /// static KEYWORDS: &[Str] = &[
    ///     Str::from_static("fn"),
    ///     Str::from_static("let"),
    /// ];
    /// ```
    #[must_use]
    #[inline]
    pub const fn from_static(s: &'static str) -> Self { Self::Static(s) }

    /// 创建或复用运行时字符串（委托给 [`ArcStr::new`]）
    ///
    /// ⚠️ 对于编译期已知的字面量，应优先使用 [`from_static`](Str::from_static)——
    /// `new()` 会将字符串送入全局池，产生不必要的哈希与锁开销。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s1 = Str::new("dynamic");
    /// let s2 = Str::new("dynamic"); // 复用 s1 的底层内存
    /// assert_eq!(s1, s2);
    /// ```
    #[inline]
    pub fn new<S: AsRef<str>>(s: S) -> Self { Self::Counted(ArcStr::new(s)) }

    /// 是否为编译期字面量变体
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// assert!(Str::from_static("lit").is_static());
    /// assert!(!Str::new("dyn").is_static());
    /// ```
    #[must_use]
    #[inline]
    pub const fn is_static(&self) -> bool { matches!(self, Self::Static(_)) }

    /// 引用计数快照
    ///
    /// - `Static` 变体返回 `None`（无引用计数概念）
    /// - `Counted` 变体返回 `Some(count)`
    ///
    /// 并发环境下返回值可能立即过时，仅用于调试与测试。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s1 = Str::from_static("a");
    /// let s2 = Str::new("b");
    /// assert_eq!(s1.ref_count(), None);
    /// assert_eq!(s2.ref_count(), Some(1));
    /// ```
    #[must_use]
    #[inline]
    pub fn ref_count(&self) -> Option<usize> {
        match self {
            Self::Static(_) => None,
            Self::Counted(arc) => Some(arc.ref_count()),
        }
    }

    /// 尝试获取 `&'static str`（仅 `Static` 变体成功）
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::from_static("literal");
    /// assert_eq!(s.as_static(), Some("literal"));
    ///
    /// let d = Str::new("dynamic");
    /// assert_eq!(d.as_static(), None);
    /// ```
    #[must_use]
    #[inline]
    pub const fn as_static(&self) -> Option<&'static str> {
        match self {
            Self::Static(s) => Some(*s),
            Self::Counted(_) => None,
        }
    }

    /// 尝试获取内部 [`ArcStr`] 引用（仅 `Counted` 变体成功）
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::new("counted");
    /// assert!(s.as_arc_str().is_some());
    ///
    /// let l = Str::from_static("literal");
    /// assert!(l.as_arc_str().is_none());
    /// ```
    #[must_use]
    #[inline]
    pub const fn as_arc_str(&self) -> Option<&ArcStr> {
        match self {
            Self::Static(_) => None,
            Self::Counted(arc) => Some(arc),
        }
    }

    /// 消费 `Str`，尝试提取内部 [`ArcStr`]
    ///
    /// `Static` 变体返回 `None`，`Counted` 变体零成本解包。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::new("extract");
    /// let arc = s.into_arc_str().unwrap();
    /// assert_eq!(arc.as_str(), "extract");
    /// ```
    #[must_use]
    #[inline]
    pub fn into_arc_str(self) -> Option<ArcStr> {
        match self {
            Self::Static(_) => None,
            Self::Counted(arc) => Some(arc),
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Method shadowing — 覆盖 Deref<Target=str> 提供的同名方法
//
// 对 Counted 变体，直接读取 ArcStrInner 内部字段，
// 跳过了 Deref → &str → 再访问的间接路径。
// 对 Static 变体，等价于直接访问 &'static str。
// ────────────────────────────────────────────────────────────────────────────

impl Str {
    /// 获取字符串切片（零成本）
    ///
    /// 两个变体均为直接内存访问，无间接寻址。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::from_static("hello");
    /// assert_eq!(s.as_str(), "hello");
    /// ```
    #[must_use]
    #[inline]
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Static(s) => s,
            Self::Counted(arc) => arc.as_str(),
        }
    }

    /// 获取底层 UTF-8 字节切片
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::new("hello");
    /// assert_eq!(s.as_bytes(), b"hello");
    /// ```
    #[must_use]
    #[inline]
    pub const fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Static(s) => s.as_bytes(),
            Self::Counted(arc) => arc.as_bytes(),
        }
    }

    /// 字符串长度（UTF-8 字节数）
    ///
    /// `Counted` 变体直接读取 `ArcStrInner::string_len`，
    /// 无需先构造 `&str`。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s = Str::new("hello");
    /// assert_eq!(s.len(), 5);
    /// ```
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        match self {
            Self::Static(s) => s.len(),
            Self::Counted(arc) => arc.len(),
        }
    }

    /// 是否为空字符串
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// assert!(Str::from_static("").is_empty());
    /// assert!(!Str::new("x").is_empty());
    /// ```
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        match self {
            Self::Static(s) => s.is_empty(),
            Self::Counted(arc) => arc.is_empty(),
        }
    }

    /// 字符串数据的裸指针（指向首字节）
    ///
    /// 可用于验证两个 `Str` 是否共享同一底层内存。
    ///
    /// # Examples
    ///
    /// ```rust
    /// use interned::Str;
    ///
    /// let s1 = Str::new("ptr");
    /// let s2 = Str::new("ptr");
    /// assert_eq!(s1.as_ptr(), s2.as_ptr());
    /// ```
    #[must_use]
    #[inline]
    pub const fn as_ptr(&self) -> *const u8 {
        match self {
            Self::Static(s) => s.as_ptr(),
            Self::Counted(arc) => arc.as_ptr(),
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Tests
// ────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_two_words() {
        assert_eq!(core::mem::size_of::<Str>(), 16);
        assert_eq!(core::mem::align_of::<Str>(), 8);
        assert!(core::mem::needs_drop::<Str>());
    }

    #[test]
    fn method_shadowing() {
        let s1 = Str::from_static("hello");
        let s2 = Str::new("world");

        assert_eq!(s1.len(), 5);
        assert_eq!(s2.len(), 5);
        assert!(!s1.is_empty());
        assert_eq!(s1.as_bytes(), b"hello");
        assert_eq!(s1.as_str(), "hello");
    }

    #[test]
    fn static_vs_counted() {
        let s1 = Str::from_static("hello");
        let s2 = Str::new("hello");

        assert!(s1.is_static());
        assert!(!s2.is_static());
        assert_eq!(s1.ref_count(), None);
        assert!(s2.ref_count().is_some());
        assert_eq!(s1, s2);
    }

    #[test]
    fn arcstr_conversions() {
        let arc = ArcStr::new("test");
        let before = arc.ref_count();

        let s: Str = arc.clone().into();
        assert!(!s.is_static());
        assert_eq!(s.ref_count(), Some(before + 1));

        let back = s.into_arc_str();
        assert!(back.is_some());
        assert_eq!(back.unwrap(), arc);
    }

    #[test]
    fn arcstr_equality() {
        let arc = ArcStr::new("same");
        let s1 = Str::from(arc.clone());
        let s2 = Str::from_static("same");

        assert_eq!(s1, arc);
        assert_eq!(s2, arc);
    }

    #[test]
    fn default_is_empty_static() {
        let s = Str::default();
        assert!(s.is_empty());
        assert!(s.is_static());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn const_construction() {
        const GREETING: Str = Str::from_static("Hello");
        static KEYWORDS: &[Str] =
            &[Str::from_static("fn"), Str::from_static("let"), Str::from_static("match")];

        assert!(GREETING.is_static());
        assert_eq!(KEYWORDS.len(), 3);
        assert!(KEYWORDS[0].is_static());
    }

    #[test]
    fn deref_to_str_methods() {
        let s = Str::from_static("deref");
        assert!(s.starts_with("de"));
        assert!(s.contains("ref"));
        assert_eq!(s.to_uppercase(), "DEREF");
    }

    #[test]
    fn ordering() {
        let mut v = vec![Str::new("cherry"), Str::from_static("apple"), Str::new("banana")];
        v.sort();
        assert_eq!(v[0], "apple");
        assert_eq!(v[1], "banana");
        assert_eq!(v[2], "cherry");
    }

    #[test]
    fn conversions() {
        let s1: Str = "literal".into();
        let s2: Str = String::from("owned").into();
        let s3: Str = ArcStr::new("arc").into();

        assert!(s1.is_static());
        assert!(!s2.is_static());
        assert!(!s3.is_static());

        let string: String = s2.clone().into();
        assert_eq!(string, "owned");

        let boxed: alloc::boxed::Box<str> = s3.into();
        assert_eq!(&*boxed, "arc");
    }

    #[test]
    fn hash_consistency() {
        use std::{
            collections::hash_map::DefaultHasher,
            hash::{Hash, Hasher},
        };

        let s1 = Str::from_static("test");
        let s2 = Str::new("test");

        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        s1.hash(&mut h1);
        s2.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }
}
