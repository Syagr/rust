use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::boxed::Box;
use std::marker::PhantomData;
// std::mem not needed

/// Simple reference-counted pointer (not as feature-complete as std::sync::Arc)
pub struct MyArc<T> {
    ptr: NonNull<ArcInner<T>>,
    _marker: PhantomData<ArcInner<T>>,
}

struct ArcInner<T> {
    refcnt: AtomicUsize,
    data: T,
}

impl<T> MyArc<T> {
    pub fn new(data: T) -> Self {
        let boxed = Box::new(ArcInner {
            refcnt: AtomicUsize::new(1),
            data,
        });
        MyArc {
            ptr: unsafe { NonNull::new_unchecked(Box::into_raw(boxed)) },
            _marker: PhantomData,
        }
    }

    fn inner(&self) -> &ArcInner<T> {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T> Clone for MyArc<T> {
    fn clone(&self) -> Self {
        let old = self.inner();
        // increment refcount (relaxed is ok for refcount increment)
        old.refcnt.fetch_add(1, Ordering::Relaxed);
        MyArc { ptr: self.ptr, _marker: PhantomData }
    }
}

impl<T> Deref for MyArc<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner().data
    }
}

impl<T> Drop for MyArc<T> {
    fn drop(&mut self) {
        let inner = self.inner();
        // decrement with release, and if it was the last, acquire fence then drop
        if inner.refcnt.fetch_sub(1, Ordering::Release) == 1 {
            std::sync::atomic::fence(Ordering::Acquire);
            unsafe {
                // reconstruct Box and drop
                drop(Box::from_raw(self.ptr.as_ptr()));
            }
        }
    }
}

// Safety: `MyArc<T>` provides shared ownership to the contained `T`.
// It's safe to send `MyArc<T>` to another thread only when `T` is both `Send` and `Sync`.
unsafe impl<T: Send + Sync> Send for MyArc<T> {}
// `MyArc<T>` can be shared between threads (i.e., &MyArc<T> is Send) when `T: Sync`.
unsafe impl<T: Sync> Sync for MyArc<T> {}

/// Simple spinlock-based mutex
pub struct MyMutex<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Send for MyMutex<T> {}
unsafe impl<T: Send> Sync for MyMutex<T> {}

pub struct MyMutexGuard<'a, T> {
    mutex: &'a MyMutex<T>,
    // prevent Send/Sync auto impls that could allow moving guard between threads unsafely
    _nosend: PhantomData<*mut ()>,
}

impl<T> MyMutex<T> {
    pub fn new(t: T) -> Self {
        MyMutex {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(t),
        }
    }

    pub fn lock(&self) -> MyMutexGuard<'_, T> {
        // simple spinlock with yielding
        while self.locked.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            // yield to avoid busy-wait burning CPU
            std::thread::yield_now();
        }
        MyMutexGuard { mutex: self, _nosend: PhantomData }
    }
}

impl<'a, T> Deref for MyMutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}
impl<'a, T> DerefMut for MyMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mutex.data.get() }
    }
}

impl<'a, T> Drop for MyMutexGuard<'a, T> {
    fn drop(&mut self) {
        // release lock
        self.mutex.locked.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn arc_clone_drop() {
        let a = MyArc::new(5);
        let b = a.clone();
        assert_eq!(*a, 5);
        assert_eq!(*b, 5);
    }

    #[test]
    fn mutex_basic() {
        let m = MyMutex::new(0);
        {
            let mut g = m.lock();
            *g += 1;
        }
        let g = m.lock();
        assert_eq!(*g, 1);
    }

    #[test]
    fn concurrent_mutex() {
        let m = std::sync::Arc::new(MyMutex::new(0usize));
        let mut handles = Vec::new();
        for _ in 0..4 {
            let m2 = std::sync::Arc::clone(&m);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let mut g = m2.lock();
                    *g += 1;
                }
            }));
        }
        for h in handles { h.join().unwrap(); }
        let g = m.lock();
        assert_eq!(*g, 4 * 1000);
    }
}