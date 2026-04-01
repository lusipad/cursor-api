//! Rkyv 零拷贝序列化支持（`feature = "rkyv"`）
//!
//! 归档格式使用 `ArchivedString`——rkyv 内置的相对指针字符串。
//! 反序列化时通过 `ArcStr::new()` 重新池化，恢复去重语义。

use super::ArcStr;
use core::cmp::Ordering;
use rkyv::{
    Archive, Deserialize, DeserializeUnsized, Place, Serialize, SerializeUnsized,
    rancor::{Fallible, Source},
    string::{ArchivedString, StringResolver},
};

// ── Archive / Serialize / Deserialize ───────────────────────────────────────

impl Archive for ArcStr {
    type Archived = ArchivedString;
    type Resolver = StringResolver;

    #[inline]
    fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedString::resolve_from_str(self.as_str(), resolver, out);
    }
}

impl<S: Fallible + ?Sized> Serialize<S> for ArcStr
where
    S::Error: Source,
    str: SerializeUnsized<S>,
{
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        ArchivedString::serialize_from_str(self.as_str(), serializer)
    }
}

impl<D: Fallible + ?Sized> Deserialize<ArcStr, D> for ArchivedString
where str: DeserializeUnsized<str, D>
{
    fn deserialize(&self, _: &mut D) -> Result<ArcStr, D::Error> { Ok(ArcStr::new(self.as_str())) }
}

// ── Cross-type comparison ───────────────────────────────────────────────────
//
// 支持归档字符串与运行时 `ArcStr` 的直接比较，
// 在增量反序列化场景中避免不必要的 `ArcStr` 构造。

impl PartialEq<ArcStr> for ArchivedString {
    #[inline]
    fn eq(&self, other: &ArcStr) -> bool { self.as_str() == other.as_str() }
}

impl PartialEq<ArchivedString> for ArcStr {
    #[inline]
    fn eq(&self, other: &ArchivedString) -> bool { other.as_str() == self.as_str() }
}

impl PartialOrd<ArcStr> for ArchivedString {
    #[inline]
    fn partial_cmp(&self, other: &ArcStr) -> Option<Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

impl PartialOrd<ArchivedString> for ArcStr {
    #[inline]
    fn partial_cmp(&self, other: &ArchivedString) -> Option<Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}
