//! Rkyv 零拷贝序列化支持（`feature = "rkyv"`）
//!
//! 归档格式使用 `ArchivedString`。
//! 反序列化通过 `Str::new()` 重新池化为 `Counted` 变体。

use super::Str;
use core::cmp::Ordering;
use rkyv::{
    Archive, Deserialize, DeserializeUnsized, Place, Serialize, SerializeUnsized,
    rancor::{Fallible, Source},
    string::{ArchivedString, StringResolver},
};

// ── Archive / Serialize / Deserialize ───────────────────────────────────────

impl Archive for Str {
    type Archived = ArchivedString;
    type Resolver = StringResolver;

    #[inline]
    fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
        ArchivedString::resolve_from_str(self.as_str(), resolver, out);
    }
}

impl<S: Fallible + ?Sized> Serialize<S> for Str
where
    S::Error: Source,
    str: SerializeUnsized<S>,
{
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        ArchivedString::serialize_from_str(self.as_str(), serializer)
    }
}

impl<D: Fallible + ?Sized> Deserialize<Str, D> for ArchivedString
where str: DeserializeUnsized<str, D>
{
    fn deserialize(&self, _: &mut D) -> Result<Str, D::Error> { Ok(Str::new(self.as_str())) }
}

// ── Cross-type comparison ───────────────────────────────────────────────────

impl PartialEq<Str> for ArchivedString {
    #[inline]
    fn eq(&self, other: &Str) -> bool { self.as_str() == other.as_str() }
}

impl PartialEq<ArchivedString> for Str {
    #[inline]
    fn eq(&self, other: &ArchivedString) -> bool { other.as_str() == self.as_str() }
}

impl PartialOrd<Str> for ArchivedString {
    #[inline]
    fn partial_cmp(&self, other: &Str) -> Option<Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}

impl PartialOrd<ArchivedString> for Str {
    #[inline]
    fn partial_cmp(&self, other: &ArchivedString) -> Option<Ordering> {
        self.as_str().partial_cmp(other.as_str())
    }
}
