//! `Str` 与标准库字符串类型之间的双向转换
//!
//! # 入方向策略
//!
//! | 来源 | 目标变体 | 原因 |
//! |------|----------|------|
//! | `&'static str` | `Static` | 零成本，保留静态生命周期 |
//! | `String` | `Counted` | 动态内容必须池化 |
//! | `ArcStr` | `Counted` | 直接包装，不增加引用计数 |
//! | `Cow<str>` | `Counted` | 无论 Borrowed / Owned 都池化 |
//! | `Box<str>` | `Counted` | 同 `String` |
//!
//! # 出方向策略
//!
//! | 目标 | Static 变体 | Counted 变体 |
//! |------|-------------|--------------|
//! | `String` | 分配 + 拷贝 | 分配 + 拷贝 |
//! | `Box<str>` | 分配 + 拷贝 | 分配 + 拷贝 |
//! | `Cow<str>` | `Borrowed`（零成本） | `Owned`（需分配） |

use super::Str;
use crate::arc_str::ArcStr;
use alloc::{borrow::Cow, boxed::Box, string::String};

// ── Into Str ────────────────────────────────────────────────────────────────

impl const From<&'static str> for Str {
    /// 字面量 → Static 变体（零成本）
    #[inline]
    fn from(s: &'static str) -> Self { Self::Static(s) }
}

impl From<String> for Str {
    /// `String` → Counted 变体（池化）
    #[inline]
    fn from(s: String) -> Self { Self::Counted(ArcStr::from(s)) }
}

impl From<&String> for Str {
    #[inline]
    fn from(s: &String) -> Self { Self::Counted(ArcStr::from(s)) }
}

impl From<ArcStr> for Str {
    /// `ArcStr` → Counted 变体（直接包装，不额外增加引用计数）
    #[inline]
    fn from(arc: ArcStr) -> Self { Self::Counted(arc) }
}

impl<'a> From<Cow<'a, str>> for Str {
    /// `Cow` → Counted 变体（Borrowed / Owned 均池化）
    #[inline]
    fn from(cow: Cow<'a, str>) -> Self { Self::Counted(ArcStr::from(cow)) }
}

impl From<Box<str>> for Str {
    #[inline]
    fn from(s: Box<str>) -> Self { Self::Counted(ArcStr::from(s)) }
}

// ── From Str ────────────────────────────────────────────────────────────────

impl From<Str> for String {
    /// 总是分配——两个变体的底层内存均不可转移给 `String`
    #[inline]
    fn from(s: Str) -> Self { s.as_str().to_owned() }
}

impl From<Str> for Box<str> {
    #[inline]
    fn from(s: Str) -> Self { s.as_str().into() }
}

impl From<Str> for Cow<'_, str> {
    /// Static → `Cow::Borrowed`（零成本），Counted → `Cow::Owned`（需分配）
    #[inline]
    fn from(s: Str) -> Self {
        match s {
            Str::Static(s) => Cow::Borrowed(s),
            Str::Counted(arc) => Cow::Owned(arc.into()),
        }
    }
}

impl<'a> const From<&'a Str> for Cow<'a, str> {
    /// 借用 → `Cow::Borrowed`（零成本）
    #[inline]
    fn from(s: &'a Str) -> Self { Cow::Borrowed(s.as_str()) }
}

// ── FromStr ─────────────────────────────────────────────────────────────────

impl core::str::FromStr for Str {
    type Err = core::convert::Infallible;

    /// 永不失败——创建 Counted 变体
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> { Ok(Self::new(s)) }
}
