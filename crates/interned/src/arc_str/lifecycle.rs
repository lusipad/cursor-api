use super::ArcStr;
use crate::{
    inner::{ArcStrInner, ThreadSafePtr},
    pool::StringPool,
};
use core::marker::PhantomData;

impl<P: StringPool> Clone for ArcStr<P> {
    #[inline]
    fn clone(&self) -> Self {
        unsafe { self.ptr.as_ref().inc_strong() }
        Self { ptr: self.ptr, _marker: PhantomData }
    }
}

impl<P: StringPool> Drop for ArcStr<P> {
    fn drop(&mut self) {
        unsafe {
            let inner = self.ptr.as_ref();

            if !inner.dec_strong() {
                return;
            }

            // 最后一个引用——从池中移除并释放内存
            let pool = P::get_pool();
            let entry =
                pool.raw_entry().from_key_hashed_nocheck_sync(inner.hash, &ThreadSafePtr(self.ptr));

            if let scc::hash_map::RawEntry::Occupied(e) = entry {
                // 双重检查：获取写入权限期间可能有并发 clone
                if inner.strong_count() != 0 {
                    return;
                }

                e.remove();

                let layout = ArcStrInner::layout_for_string_unchecked(inner.string_len);
                P::deallocate(self.ptr.cast().as_ptr(), layout);
            }
        }
    }
}
