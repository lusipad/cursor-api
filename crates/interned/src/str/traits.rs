//! 标准库 trait 实现
//!
//! # 相等性策略
//!
//! - `Str` vs `Str`：
//!   - Counted vs Counted → 指针比较 O(1)（池不变量）
//!   - 其余组合 → 内容比较 O(n)
//!
//! - `Str` vs `str` / `String` / `ArcStr` → 内容比较 O(n)，
//!   但 `Str::Counted` vs `ArcStr` 可走指针快速路径。
//!
//! # 哈希一致性
//!
//! `Hash` 实现基于字符串内容（与 `str` 一致），
//! 确保 `HashMap<Str, V>` 可用 `&str` 查找（配合 `Borrow<str>`）。
//!
//! # 排序
//!
//! 总是基于字符串内容的字典序，与变体类型无关。

use super::Str;
use crate::arc_str::ArcStr;
use core::{
    borrow::Borrow,
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};

// ── Equality ────────────────────────────────────────────────────────────────

impl PartialEq for Str {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // 两个 Counted：利用 ArcStr 的 O(1) 指针比较
            (Self::Counted(a), Self::Counted(b)) => a == b,
            // 涉及 Static：退化为内容比较
            _ => self.as_str() == other.as_str(),
        }
    }
}

impl Eq for Str {}

impl const PartialEq<str> for Str {
    #[inline]
    fn eq(&self, other: &str) -> bool { self.as_str() == other }
}

impl const PartialEq<&str> for Str {
    #[inline]
    fn eq(&self, other: &&str) -> bool { self.as_str() == *other }
}

impl const PartialEq<String> for Str {
    #[inline]
    fn eq(&self, other: &String) -> bool { self.as_str() == other.as_str() }
}

impl const PartialEq<Str> for str {
    #[inline]
    fn eq(&self, other: &Str) -> bool { self == other.as_str() }
}

impl const PartialEq<Str> for &str {
    #[inline]
    fn eq(&self, other: &Str) -> bool { *self == other.as_str() }
}

impl const PartialEq<Str> for String {
    #[inline]
    fn eq(&self, other: &Str) -> bool { self.as_str() == other.as_str() }
}

impl PartialEq<ArcStr> for Str {
    /// Counted 变体走指针快速路径，Static 变体走内容比较
    #[inline]
    fn eq(&self, other: &ArcStr) -> bool {
        match self {
            Self::Counted(arc) => arc == other,
            Self::Static(s) => *s == other.as_str(),
        }
    }
}

impl PartialEq<Str> for ArcStr {
    #[inline]
    fn eq(&self, other: &Str) -> bool { other == self }
}

// ── Ordering ────────────────────────────────────────────────────────────────

impl PartialOrd for Str {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Ord for Str {
    /// 字典序，与变体类型无关
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering { self.as_str().cmp(other.as_str()) }
}

// ── Hashing ─────────────────────────────────────────────────────────────────

impl Hash for Str {
    /// 基于字符串内容哈希——保证 `Static("a")` 与 `Counted("a")` 哈希值相同
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) { self.as_str().hash(state) }
}

// ── Deref / AsRef / Borrow ──────────────────────────────────────────────────

impl core::ops::Deref for Str {
    type Target = str;

    /// 自动解引用——让 `Str` 透明获得 `str` 的全部方法
    ///
    /// 注意：`len()` / `is_empty()` / `as_bytes()` 等常用方法
    /// 已被 `Str` 自身的同名方法覆盖（method shadowing），
    /// 直接调用时走的是优化路径，不会经过此 `Deref`。
    #[inline]
    fn deref(&self) -> &Self::Target { self.as_str() }
}

impl const AsRef<str> for Str {
    #[inline]
    fn as_ref(&self) -> &str { self.as_str() }
}

impl const AsRef<[u8]> for Str {
    #[inline]
    fn as_ref(&self) -> &[u8] { self.as_bytes() }
}

impl const Borrow<str> for Str {
    /// 使 `HashMap<Str, V>` 支持 `.get("key")` 查找
    #[inline]
    fn borrow(&self) -> &str { self.as_str() }
}

// ── Formatting ──────────────────────────────────────────────────────────────

impl fmt::Display for Str {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str(self.as_str()) }
}

impl fmt::Debug for Str {
    /// 调试输出区分变体：
    /// - `Str::Static("content")`
    /// - `Str::Counted("content", refcount=N)`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static(s) => f.debug_tuple("Str::Static").field(s).finish(),
            Self::Counted(arc) => f
                .debug_tuple("Str::Counted")
                .field(&arc.as_str())
                .field(&format_args!("refcount={}", arc.ref_count()))
                .finish(),
        }
    }
}

// ── Default ─────────────────────────────────────────────────────────────────

impl const Default for Str {
    /// 空字符串的 Static 变体——零分配
    #[inline]
    fn default() -> Self { Self::Static(Default::default()) }
}
