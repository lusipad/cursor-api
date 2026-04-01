pub const trait IsDefault: Sized {
    fn is_default(&self) -> bool;
}

macro_rules! impl_is_default_prim {
    ($($ty:ty : $zero:expr),* $(,)?) => {$(
        impl const IsDefault for $ty {
            #[inline(always)]
            fn is_default(&self) -> bool { *self == $zero }
        }
    )*};
}

impl_is_default_prim! {
    bool: false,
    i32: 0, i64: 0, u32: 0, u64: 0,
    f32: 0.0, f64: 0.0,
}

#[cfg(feature = "alloc")]
impl const IsDefault for ::alloc::string::String {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

#[cfg(feature = "byte_str")]
impl const IsDefault for ::byte_str::ByteStr {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

#[cfg(feature = "bytes")]
impl const IsDefault for ::bytes::Bytes {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

#[cfg(feature = "alloc")]
impl<T> const IsDefault for ::alloc::vec::Vec<T> {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

impl<T> const IsDefault for crate::Enum<T> {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.get().is_default()
    }
}

impl<B: [const] IsDefault> const IsDefault for crate::Bytes<B> {
    #[inline(always)]
    fn is_default(&self) -> bool { self.0.is_default() }
}

impl<T> const IsDefault for ::core::option::Option<T> {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_none()
    }
}

#[cfg(feature = "std")]
impl<K, V, S> IsDefault for std::collections::HashMap<K, V, S> {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

#[cfg(feature = "alloc")]
impl<K, V, A: ::alloc::alloc::Allocator + Clone> IsDefault for ::alloc::collections::BTreeMap<K, V, A> {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}

#[cfg(feature = "indexmap")]
impl<K, V, S> IsDefault for ::indexmap::IndexMap<K, V, S> {
    #[inline(always)]
    fn is_default(&self) -> bool {
        self.is_empty()
    }
}
