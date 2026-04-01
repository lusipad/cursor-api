//! `ArcStr` 与标准库字符串类型之间的双向转换
//!
//! # 设计原则
//!
//! - **入方向** (`From<X> for ArcStr`)：全部委托给 `ArcStr::new()`，
//!   确保所有字符串都经过池化去重。
//!
//! - **出方向** (`From<ArcStr> for X`)：必然涉及内存拷贝，
//!   因为 `ArcStr` 的底层内存由池管理，不可移交所有权。
//!
//! - `FromStr`：永不失败（`Err = Infallible`），
//!   使 `"text".parse::<ArcStr>()` 成立。

use super::ArcStr;
use alloc::{borrow::Cow, boxed::Box, string::String};

// ── Into ArcStr ─────────────────────────────────────────────────────────────

impl<'a> From<&'a str> for ArcStr {
    #[inline]
    fn from(s: &'a str) -> Self { Self::new(s) }
}

impl<'a> From<&'a String> for ArcStr {
    #[inline]
    fn from(s: &'a String) -> Self { Self::new(s) }
}

impl From<String> for ArcStr {
    /// `String` 的堆内存在池化后不再需要，由 `String::drop` 回收。
    /// 如果池中已有相同内容，则 `String` 的内存是纯浪费——
    /// 调用方在热路径上应优先使用 `ArcStr::new(&str)` 避免多余分配。
    #[inline]
    fn from(s: String) -> Self { Self::new(s) }
}

impl<'a> From<Cow<'a, str>> for ArcStr {
    #[inline]
    fn from(cow: Cow<'a, str>) -> Self { Self::new(cow) }
}

impl From<Box<str>> for ArcStr {
    #[inline]
    fn from(s: Box<str>) -> Self { Self::new(s) }
}

// ── From ArcStr ─────────────────────────────────────────────────────────────

impl From<ArcStr> for String {
    /// 总是分配新的 `String`——`ArcStr` 的底层内存不可转移。
    #[inline]
    fn from(s: ArcStr) -> Self { s.as_str().to_owned() }
}

impl From<ArcStr> for Box<str> {
    #[inline]
    fn from(s: ArcStr) -> Self { s.as_str().into() }
}

// ── FromStr ─────────────────────────────────────────────────────────────────

impl core::str::FromStr for ArcStr {
    type Err = core::convert::Infallible;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> { Ok(Self::new(s)) }
}
