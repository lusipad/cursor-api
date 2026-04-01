use std::cell::UnsafeCell;
use std::hint::unreachable_unchecked;
use std::pin::{Pin, pin};
use std::ptr;
use std::sync::atomic::Ordering;
use std::sync::atomic::Ordering::Acquire;

use saa::lock::Mode;
use saa::{Lock, Pager};
use sdd::{AtomicRaw, Owned, RawPtr};

use crate::Guard;

/// [`AsyncGuard`] is used when an asynchronous task needs to be suspended without invalidating any
/// references.
///
/// The validity of those references must be checked and verified by the user.
#[derive(Default)]
pub(crate) struct AsyncGuard {
    /// [`Guard`] that can be dropped without invalidating any references.
    guard: UnsafeCell<Option<Guard>>,
}

pub(crate) struct AsyncPager {
    /// Allows the user to await the lock anywhere in the code.
    pager: Pager<'static, Lock>,
}

/// [`LockPager`] enables asynchronous code to remotely wait for a [`Lock`].
pub(crate) trait LockPager {
    /// Registers the [`Pager`] in the [`Lock`], or synchronously waits for the [`Lock`] to be
    /// available.
    ///
    /// Returns `true` if the thread can retry the operation in-place.
    #[must_use]
    fn try_wait<const READ: bool>(&mut self, lock: &Lock) -> bool;

    /// Tries to acquire the [`Lock`] synchronously, or registers the [`Pager`] in the [`Lock`] and
    /// returns an error.
    fn try_acquire<const READ: bool>(&mut self, lock: &Lock) -> Result<bool, ()>;
}

/// Dereferences a pointer without checking the tag bits.
#[inline]
pub(crate) const fn deref_unchecked<T>(ptr: RawPtr<'_, T>) -> Option<&'_ T> {
    unsafe { ptr.into_ptr().as_ref_unchecked() }
}

/// Unwraps an optional value without checking the state.
#[inline]
pub(crate) const fn unwrap_unchecked<T>(v: Option<T>) -> T {
    unsafe { v.unwrap_unchecked() }
}

/// Takes the current snapshot of the value.
#[inline]
pub(crate) const fn take_snapshot<T>(v: &T) -> T {
    unsafe { ptr::from_ref(v).read() }
}

/// Gets an [`Owned`] from an [`RawPtr`].
#[inline]
pub(crate) fn get_owned<T>(a: RawPtr<'_, T>) -> Option<Owned<T>> {
    unsafe { Owned::from_raw(a) }
}

/// Returns a fake reference for passing a reference to `U` when it is ensured that the returned
/// reference is never used.
#[inline]
pub(crate) const fn fake_ref<'l, T, U>(v: &T) -> &'l U {
    unsafe { &*ptr::from_ref(v).cast::<U>() }
}

/// Hint indicating that the condition is likely to be true.
#[allow(clippy::inline_always)]
#[inline(always)]
pub const fn likely(cond: bool) -> bool {
    if cond {
        true
    } else {
        #[cold]
        #[inline]
        const fn cold_path() {}
        cold_path();
        false
    }
}

/// Marker indicating that execution of any code following it is undefined behavior.
#[inline]
pub(crate) const fn undefined() -> ! {
    unsafe { unreachable_unchecked() }
}

impl AsyncGuard {
    /// Returns `true` if the [`AsyncGuard`] contains a valid [`Guard`].
    #[inline]
    pub(crate) const fn has_guard(&self) -> bool {
        unsafe { (*self.guard.get()).is_some() }
    }

    /// Returns or creates a new [`Guard`].
    ///
    /// # Safety
    ///
    /// The caller must ensure that any references derived from the returned [`Guard`] do not
    /// outlive the underlying instance.
    #[inline]
    pub(crate) fn guard(&self) -> &Guard {
        unsafe { (*self.guard.get()).get_or_insert_with(Guard::new) }
    }

    /// Resets the [`AsyncGuard`] to its initial state.
    #[inline]
    pub(crate) fn reset(&self) {
        unsafe {
            *self.guard.get() = None;
        }
    }

    /// Loads the value of the [`AtomicRaw`] without exposing the [`Guard`] or checking tag bits.
    #[inline]
    pub(crate) fn load_unchecked<T>(&self, atomic_ptr: &AtomicRaw<T>, mo: Ordering) -> Option<&T> {
        deref_unchecked(atomic_ptr.load(mo, self.guard()))
    }

    /// Checks if the reference is valid.
    #[inline]
    pub(crate) fn check_ref<T>(&self, atomic_ptr: &AtomicRaw<T>, r: &T, mo: Ordering) -> bool {
        self.load_unchecked(atomic_ptr, mo)
            .is_some_and(|s| ptr::eq(s, r))
    }
}

// SAFETY: this is the sole purpose of `AsyncGuard`; Send-safety should be ensured by the user,
// e.g., the `AsyncGuard` should always be reset before the task is suspended.
unsafe impl Send for AsyncGuard {}
unsafe impl Sync for AsyncGuard {}

impl AsyncPager {
    /// Awaits the [`Lock`] to be available.
    #[inline]
    pub async fn wait(self: &mut Pin<&mut Self>) {
        let this = unsafe { ptr::read(self) };
        let mut pinned_pager = unsafe { Pin::new_unchecked(&mut this.get_unchecked_mut().pager) };
        let _result = pinned_pager.poll_async().await;
    }
}

impl Default for AsyncPager {
    #[inline]
    fn default() -> Self {
        Self {
            pager: unsafe {
                std::mem::transmute::<Pager<'_, Lock>, Pager<'static, Lock>>(Pager::default())
            },
        }
    }
}

impl LockPager for Pin<&mut AsyncPager> {
    #[inline]
    fn try_wait<const READ: bool>(&mut self, lock: &Lock) -> bool {
        let this = unsafe { ptr::read(self) };
        let mut pinned_pager = unsafe {
            let pager_ref = std::mem::transmute::<&mut Pager<'static, Lock>, &mut Pager<Lock>>(
                &mut this.get_unchecked_mut().pager,
            );
            Pin::new_unchecked(pager_ref)
        };
        let mode = if READ {
            Mode::WaitShared
        } else {
            Mode::WaitExclusive
        };
        lock.register_pager(&mut pinned_pager, mode, false);
        false
    }

    #[inline]
    fn try_acquire<const READ: bool>(&mut self, lock: &Lock) -> Result<bool, ()> {
        if (READ && lock.try_share()) || (!READ && lock.try_lock()) {
            return Ok(true);
        } else if lock.is_poisoned(Acquire) {
            return Ok(false);
        }
        let _: bool = self.try_wait::<READ>(lock);
        Err(())
    }
}

impl LockPager for () {
    #[inline]
    fn try_wait<const READ: bool>(&mut self, lock: &Lock) -> bool {
        let mut pinned_pager = pin!(Pager::default());
        let mode = if READ {
            Mode::WaitShared
        } else {
            Mode::WaitExclusive
        };
        lock.register_pager(&mut pinned_pager, mode, true);
        pinned_pager.poll_sync().is_ok_and(|r| r)
    }

    #[inline]
    fn try_acquire<const READ: bool>(&mut self, lock: &Lock) -> Result<bool, ()> {
        Ok((READ && lock.share_sync()) || (!READ && lock.lock_sync()))
    }
}
