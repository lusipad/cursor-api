use super::ArcStr;
use crate::pool::StringPool;
use core::{
    borrow::Borrow,
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
};

// ── Equality ────────────────────────────────────────────────────────────────
//
// 池的不变量保证：相同内容 ⟹ 相同指针地址。
// 因此 `ArcStr` 之间的相等比较可以退化为 O(1) 的指针比较，
// 而与 `str` / `String` 的跨类型比较仍需逐字节对比。

impl<P: StringPool> PartialEq for ArcStr<P> {
    /// O(1) 指针比较——池保证内容相同的字符串共享同一地址
    #[inline]
    fn eq(&self, other: &Self) -> bool { self.ptr == other.ptr }
}

impl Eq for ArcStr {}

impl const PartialEq<str> for ArcStr {
    #[inline]
    fn eq(&self, other: &str) -> bool { self.as_str() == other }
}

impl const PartialEq<&str> for ArcStr {
    #[inline]
    fn eq(&self, other: &&str) -> bool { self.as_str() == *other }
}

impl const PartialEq<ArcStr> for str {
    #[inline]
    fn eq(&self, other: &ArcStr) -> bool { self == other.as_str() }
}

impl const PartialEq<ArcStr> for &str {
    #[inline]
    fn eq(&self, other: &ArcStr) -> bool { *self == other.as_str() }
}

impl const PartialEq<String> for ArcStr {
    #[inline]
    fn eq(&self, other: &String) -> bool { self.as_str() == other.as_str() }
}

impl const PartialEq<ArcStr> for String {
    #[inline]
    fn eq(&self, other: &ArcStr) -> bool { self.as_str() == other.as_str() }
}

// ── Ordering ────────────────────────────────────────────────────────────────
//
// 排序必须基于字符串内容（字典序），而非指针地址。
// 指针地址取决于分配顺序，与字典序无关。

impl PartialOrd for ArcStr {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl Ord for ArcStr {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering { self.as_str().cmp(other.as_str()) }
}

impl PartialOrd<str> for ArcStr {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<Ordering> { Some(self.as_str().cmp(other)) }
}

impl PartialOrd<String> for ArcStr {
    #[inline]
    fn partial_cmp(&self, other: &String) -> Option<Ordering> {
        Some(self.as_str().cmp(other.as_str()))
    }
}

// ── Hashing ─────────────────────────────────────────────────────────────────
//
// 虽然 `ArcStrInner` 内预存了 ahash 计算的哈希值，
// 但标准库的 Hash trait 必须与 `str` / `String` 保持一致——
// 即使用调用方提供的 Hasher 对字符串内容重新哈希。
//
// 这确保了 `HashMap<ArcStr, V>` 可以用 `&str` 作为查找键
// （配合 `Borrow<str>` 实现），两者的哈希值相等。

impl Hash for ArcStr {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) { self.as_str().hash(state) }
}

// ── Formatting ──────────────────────────────────────────────────────────────

impl fmt::Display for ArcStr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { fmt::Display::fmt(self.as_str(), f) }
}

impl fmt::Debug for ArcStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Debug::fmt(self.as_str(), f) }
}

// ── Deref / AsRef / Borrow ──────────────────────────────────────────────────
//
// 这三组 trait 构成了 Rust 字符串类型互操作的基石：
//
// - `Deref<Target=str>` ：让 `ArcStr` 自动获得 `str` 的全部方法
// - `AsRef<str>` / `AsRef<[u8]>` ：泛型函数参数 `impl AsRef<str>` 兼容
// - `Borrow<str>` ：`HashMap<ArcStr, V>` 可用 `&str` 查找
//     （标准库要求 `Borrow` 的实现与 `Hash` + `Eq` 一致）

impl const core::ops::Deref for ArcStr {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target { self.as_str() }
}

impl const AsRef<str> for ArcStr {
    #[inline]
    fn as_ref(&self) -> &str { self.as_str() }
}

impl const AsRef<[u8]> for ArcStr {
    #[inline]
    fn as_ref(&self) -> &[u8] { self.as_bytes() }
}

impl const Borrow<str> for ArcStr {
    #[inline]
    fn borrow(&self) -> &str { self.as_str() }
}
