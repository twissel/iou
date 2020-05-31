use crate::{IoUring, SubmissionQueueEvent};
use parking_lot::Mutex;
use std::io;
use std::marker::PhantomData;
use std::ptr::{self, NonNull};

pub struct SubmissionQueue<'ring> {
    lock: Mutex<()>,
    ring: NonNull<uring_sys::io_uring>,
    _marker: PhantomData<&'ring IoUring>,
}

unsafe impl Send for SubmissionQueue<'_> {}
unsafe impl Sync for SubmissionQueue<'_> {}

impl<'ring> SubmissionQueue<'ring> {
    pub(crate) fn new(ring: &'ring IoUring) -> SubmissionQueue<'ring> {
        SubmissionQueue {
            lock: Mutex::new(()),
            ring: NonNull::from(&ring.ring),
            _marker: PhantomData,
        }
    }

    pub fn next_sqe(&self) -> Option<SubmissionQueueEvent<'_>> {
        unsafe {
            let guard = self.lock.lock();
            let sqe = uring_sys::io_uring_get_sqe(self.ring.as_ptr());
            drop(guard);
            if sqe != ptr::null_mut() {
                let mut sqe = SubmissionQueueEvent::new(&mut *sqe);
                sqe.clear();
                Some(sqe)
            } else {
                None
            }
        }
    }

    pub fn submit(&self) -> io::Result<usize> {
        let guard = self.lock.lock();
        unsafe {
            let submitted = uring_sys::io_uring_flush_sq(self.ring.as_ptr());
            drop(guard);
            resultify!(uring_sys::io_uring_submit_raw(
                self.ring.as_ptr(),
                submitted as _,
                0
            ))
        }
    }
}
