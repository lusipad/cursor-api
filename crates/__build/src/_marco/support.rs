use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

pub(crate) struct Variable<T>(pub(crate) UnsafeCell<MaybeUninit<T>>);

unsafe impl<T> Send for Variable<T> {}
unsafe impl<T> Sync for Variable<T> {}

#[inline(always)]
pub(crate) const fn uninit_variable<T>() -> Variable<T> {
  Variable(UnsafeCell::new(MaybeUninit::uninit()))
}
