use core::{
    alloc::Layout,
    hint,
    ptr::NonNull,
    sync::atomic::{
        AtomicUsize,
        Ordering::{Relaxed, Release},
    },
};

/// 字符串内容的内部表示（DST 头部）
///
/// # 内存布局
///
/// ```text
/// [hash:u64][count:AtomicUsize][len:usize][string_data...]
/// ```
#[repr(C)]
pub(crate) struct ArcStrInner {
    pub(crate) hash: u64,
    count: AtomicUsize,
    pub(crate) string_len: usize,
}

impl ArcStrInner {
    const MAX_LEN: usize = isize::MAX as usize - core::mem::size_of::<Self>();

    #[must_use]
    #[inline]
    pub(crate) const unsafe fn string_ptr(&self) -> *const u8 {
        core::ptr::from_ref(self).add(1).cast()
    }

    #[must_use]
    #[inline]
    pub(crate) const unsafe fn as_bytes(&self) -> &[u8] {
        core::slice::from_raw_parts(self.string_ptr(), self.string_len)
    }

    #[must_use]
    #[inline]
    pub(crate) const unsafe fn as_str(&self) -> &str {
        core::str::from_utf8_unchecked(self.as_bytes())
    }

    pub(crate) fn layout_for_string(string_len: usize) -> Layout {
        if string_len > Self::MAX_LEN {
            hint::cold_path();
            panic!("字符串过长: {string_len} 字节 (最大支持: {})", Self::MAX_LEN);
        }
        unsafe { Self::layout_for_string_unchecked(string_len) }
    }

    pub(crate) const unsafe fn layout_for_string_unchecked(string_len: usize) -> Layout {
        let header = Layout::new::<Self>();
        let string_data = Layout::from_size_align_unchecked(string_len, 1);
        let (combined, _) = header.extend(string_data).unwrap_unchecked();
        combined.pad_to_align()
    }

    pub(crate) const unsafe fn write_with_string(ptr: NonNull<Self>, string: &str, hash: u64) {
        let inner = ptr.as_ptr();
        core::ptr::write(
            inner,
            Self { hash, count: AtomicUsize::new(1), string_len: string.len() },
        );
        let dst = (*inner).string_ptr().cast_mut();
        core::ptr::copy_nonoverlapping(string.as_ptr(), dst, string.len());
    }

    #[inline]
    pub(crate) unsafe fn inc_strong(&self) {
        let old = self.count.fetch_add(1, Relaxed);
        if old > isize::MAX as usize {
            hint::cold_path();
            core::intrinsics::abort();
        }
    }

    #[inline]
    pub(crate) unsafe fn dec_strong(&self) -> bool { self.count.fetch_sub(1, Release) == 1 }

    #[inline]
    pub(crate) fn strong_count(&self) -> usize { self.count.load(Relaxed) }
}
