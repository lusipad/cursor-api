use crate::app::constant::UNNAMED_PATTERN;
use alloc::{
    borrow::{Borrow, Cow},
    sync::Arc,
};
use core::fmt;

#[derive(Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Alias(Arc<str>);

impl Alias {
    #[inline]
    pub fn new<'a, S: Into<Cow<'a, str>>>(s: S) -> Self {
        let s: Cow<'_, str> = s.into();
        Self(s.into())
    }

    #[inline]
    pub fn is_unnamed(&self) -> bool { self.0.starts_with(UNNAMED_PATTERN) }

    #[inline]
    pub fn into_inner(self) -> Arc<str> { self.0 }
}

impl Borrow<str> for Alias {
    #[inline]
    fn borrow(&self) -> &str { &self.0 }
}

impl fmt::Display for Alias {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.borrow()) }
}

impl ::serde::Serialize for Alias {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: ::serde::Serializer {
        serializer.serialize_str(self.borrow())
    }
}
