use core::{
    cmp, fmt,
    hash::{Hash, Hasher},
    marker::Destruct,
    ops::{Deref, DerefMut},
};

#[repr(transparent)]
pub struct Bytes<B>(pub B);

impl<B> Bytes<B> {
    #[inline]
    fn slice_ref(data: &[Self]) -> &[B] { unsafe { core::mem::transmute(data) } }
}

impl<B: Clone> Clone for Bytes<B> {
    #[inline]
    fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl<B: Copy> Copy for Bytes<B> {}

impl<B: [const] PartialEq> const PartialEq for Bytes<B> {
    #[inline]
    fn eq(&self, other: &Self) -> bool { PartialEq::eq(&self.0, &other.0) }
    #[inline]
    fn ne(&self, other: &Self) -> bool { PartialEq::ne(&self.0, &other.0) }
}

impl<B: [const] Eq> const Eq for Bytes<B> {}

impl<B: [const] PartialOrd> const PartialOrd for Bytes<B> {
    #[inline]
    fn partial_cmp(&self, other: &Bytes<B>) -> Option<cmp::Ordering> {
        PartialOrd::partial_cmp(&self.0, &other.0)
    }
    #[inline]
    fn lt(&self, other: &Bytes<B>) -> bool { PartialOrd::lt(&self.0, &other.0) }
    #[inline]
    fn le(&self, other: &Bytes<B>) -> bool { PartialOrd::le(&self.0, &other.0) }
    #[inline]
    fn gt(&self, other: &Bytes<B>) -> bool { PartialOrd::gt(&self.0, &other.0) }
    #[inline]
    fn ge(&self, other: &Bytes<B>) -> bool { PartialOrd::ge(&self.0, &other.0) }
}

impl<B: [const] Ord + [const] Destruct> const Ord for Bytes<B> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering { Ord::cmp(&self.0, &other.0) }
    #[inline]
    fn max(self, other: Self) -> Self
    where Self: Sized + [const] Destruct {
        Self(Ord::max(self.0, other.0))
    }
    #[inline]
    fn min(self, other: Self) -> Self
    where Self: Sized + [const] Destruct {
        Self(Ord::min(self.0, other.0))
    }
    #[inline]
    fn clamp(self, min: Self, max: Self) -> Self
    where Self: Sized + [const] Destruct {
        Self(Ord::clamp(self.0, min.0, max.0))
    }
}

impl<B: Hash> Hash for Bytes<B> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) { self.0.hash(state) }
    #[inline]
    fn hash_slice<H: Hasher>(data: &[Self], state: &mut H)
    where B: Sized {
        B::hash_slice(Self::slice_ref(data), state)
    }
}

impl<B: fmt::Debug> fmt::Debug for Bytes<B> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}

impl<B: [const] Default> const Default for Bytes<B> {
    #[inline]
    fn default() -> Self { Self(B::default()) }
}

impl<B> Deref for Bytes<B> {
    type Target = B;
    #[inline]
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl<B> DerefMut for Bytes<B> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl<B> const From<B> for Bytes<B> {
    #[inline]
    fn from(value: B) -> Self { Self(value) }
}

#[cfg(feature = "serde")]
mod serde_impls {
    use super::Bytes;
    use base64_simd::{STANDARD, forgiving_decode_to_vec};
    use core::marker::PhantomData;
    use serde_core::{
        Deserialize, Deserializer, Serialize, Serializer,
        de::{self, Unexpected, Visitor},
    };

    impl<B: AsRef<[u8]>> Serialize for Bytes<B> {
        #[inline]
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer {
            serializer.serialize_str(&STANDARD.encode_to_string(self.0.as_ref()))
        }
    }

    impl<'de, B: From<::alloc::vec::Vec<u8>>> Deserialize<'de> for Bytes<B> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de> {
            struct BytesVisitor<B>(PhantomData<B>);

            impl<'de, B: From<::alloc::vec::Vec<u8>>> Visitor<'de> for BytesVisitor<B> {
                type Value = Bytes<B>;

                fn expecting(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    f.write_str("a Base64 encoded string")
                }

                fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where E: de::Error {
                    let b = forgiving_decode_to_vec(v.as_bytes())
                        .map_err(|_| de::Error::invalid_value(Unexpected::Str(v), &self))?;
                    Ok(Bytes(b.into()))
                }
            }

            deserializer.deserialize_string(BytesVisitor(PhantomData))
        }
    }
}
