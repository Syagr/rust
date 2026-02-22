use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::sync::{Arc, Mutex};
use std::time::{Instant, Duration};

/// Wraps an inner future and records start time; when inner completes
/// it prints elapsed time to stdout.
pub struct MeasurableFuture<Fut> {
    pub inner_future: Fut,
    pub started_at: Option<Instant>,
}

impl<Fut> MeasurableFuture<Fut> {
    pub fn new(f: Fut) -> Self { Self { inner_future: f, started_at: None } }
}

impl<Fut> Future for MeasurableFuture<Fut>
where
    Fut: Future,
{
    type Output = Fut::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Safe projection without requiring Fut: Unpin — we only borrow inner by mutable reference
        let this = unsafe { self.get_unchecked_mut() };
        if this.started_at.is_none() {
            this.started_at = Some(Instant::now());
        }
        // Create a pinned reference to the inner future
        let inner = unsafe { Pin::new_unchecked(&mut this.inner_future) };
        match inner.poll(cx) {
            Poll::Ready(out) => {
                if let Some(start) = this.started_at.take() {
                    let elapsed = start.elapsed();
                    println!("MeasurableFuture elapsed: {:?}", elapsed);
                }
                Poll::Ready(out)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// TimerFuture becomes ready after `millis` milliseconds. It does not block the
/// executor thread; a background std thread sleeps then wakes the stored waker.
pub struct TimerFuture {
    shared: Arc<Mutex<TimerState>>,
}

struct TimerState {
    completed: bool,
    waker: Option<Waker>,
}

impl TimerFuture {
    pub fn new(millis: u64) -> Self {
        let state = Arc::new(Mutex::new(TimerState { completed: false, waker: None }));
        let thread_state = state.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(millis));
            let mut st = thread_state.lock().unwrap();
            st.completed = true;
            if let Some(w) = st.waker.take() {
                w.wake();
            }
        });
        TimerFuture { shared: state }
    }
}

impl Future for TimerFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut st = self.shared.lock().unwrap();
        if st.completed {
            Poll::Ready(())
        } else {
            // store latest waker so background thread can wake
            st.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::{RawWaker, RawWakerVTable, Waker, Context};
    use std::pin::Pin;

    fn noop_raw_waker() -> RawWaker {
        unsafe fn clone(_: *const ()) -> RawWaker { noop_raw_waker() }
        unsafe fn wake(_: *const ()) {}
        unsafe fn wake_by_ref(_: *const ()) {}
        unsafe fn drop(_: *const ()) {}
        RawWaker::new(std::ptr::null(), &RawWakerVTable::new(clone, wake, wake_by_ref, drop))
    }

    fn noop_waker() -> Waker { unsafe { Waker::from_raw(noop_raw_waker()) } }

    fn block_on<F: Future>(mut f: Pin<&mut F>) -> F::Output {
        let w = noop_waker();
        let mut cx = Context::from_waker(&w);
        loop {
            match f.as_mut().poll(&mut cx) {
                Poll::Ready(v) => return v,
                Poll::Pending => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    }

    #[test]
    fn timer_future_completes() {
        let mut t = Box::pin(TimerFuture::new(30));
        let start = Instant::now();
        block_on(t.as_mut());
        assert!(start.elapsed() >= Duration::from_millis(30));
    }

    #[test]
    fn measurable_future_reports_and_completes() {
        let inner = TimerFuture::new(20);
        let mut mf = Box::pin(MeasurableFuture::new(inner));
        block_on(mf.as_mut());
        // completion implies MeasurableFuture printed elapsed; test ensures no panic
    }
}
