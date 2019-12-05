use std::io;
use std::mem;
use std::os::unix::io::RawFd;
use std::ptr::{self, NonNull};
use std::marker::PhantomData;
use std::time::Duration;

use super::IoUring;
use uring_sys::__kernel_timespec;

pub struct SubmissionQueue<'ring> {
    ring: NonNull<uring_sys::io_uring>,
    _marker: PhantomData<&'ring mut IoUring>,
}

impl<'ring> SubmissionQueue<'ring> {
    pub(crate) fn new(ring: &'ring IoUring) -> SubmissionQueue<'ring> {
        SubmissionQueue {
            ring: NonNull::from(&ring.ring),
            _marker: PhantomData,
        }
    }

    pub fn next_sqe<'a>(&'a mut self) -> Option<SubmissionQueueEvent<'a>> {
        unsafe {
            let sqe = uring_sys::io_uring_get_sqe(self.ring.as_ptr());
            if sqe != ptr::null_mut() {
                let mut sqe = SubmissionQueueEvent::new(&mut *sqe);
                sqe.clear();
                Some(sqe)
            } else {
                None
            }
        }
    }

    pub fn submit(&mut self) -> io::Result<usize> {
        resultify!(unsafe { uring_sys::io_uring_submit(self.ring.as_ptr()) })
    }

    pub fn submit_and_wait(&mut self, wait_for: u32) -> io::Result<usize> {
        resultify!(unsafe { uring_sys::io_uring_submit_and_wait(self.ring.as_ptr(), wait_for as _) })
    }

    pub fn submit_and_wait_with_timeout(&mut self, wait_for: u32, duration: Duration)
        -> io::Result<usize>
    {
        let ts = uring_sys::__kernel_timespec {
            tv_sec: duration.as_secs() as _,
            tv_nsec: duration.subsec_nanos() as _
        };

        loop {
            if let Some(mut sqe) = self.next_sqe() {
                sqe.clear();
                unsafe {
                    sqe.prep_timeout(&ts);
                    return resultify!(uring_sys::io_uring_submit_and_wait(self.ring.as_ptr(), wait_for as _))
                }
            }

            self.submit()?;
        }
    }
}

unsafe impl<'ring> Send for SubmissionQueue<'ring> { }
unsafe impl<'ring> Sync for SubmissionQueue<'ring> { }

pub struct SubmissionQueueEvent<'a> {
    sqe: &'a mut uring_sys::io_uring_sqe,
}

impl<'a> SubmissionQueueEvent<'a> {
    pub(crate) fn new(sqe: &'a mut uring_sys::io_uring_sqe) -> SubmissionQueueEvent<'a> {
        SubmissionQueueEvent { sqe }
    }

    pub fn user_data(&self) -> u64 {
        self.sqe.user_data as u64
    }

    pub fn set_user_data(&mut self, user_data: u64) {
        self.sqe.user_data = user_data as _;
    }

    pub fn flags(&self) -> SubmissionFlags {
        unsafe { SubmissionFlags::from_bits_unchecked(self.sqe.flags as _) }
    }

    pub fn set_flags(&mut self, flags: SubmissionFlags) {
        self.sqe.flags = flags.bits() as _;
    }

    #[inline]
    pub unsafe fn prep_read_vectored(
        &mut self,
        fd: RawFd,
        bufs: &mut [io::IoSliceMut<'_>],
        offset: usize,
    ) {
        let len = bufs.len();
        let addr = bufs.as_mut_ptr();
        uring_sys::io_uring_prep_readv(self.sqe, fd, addr as _, len as _, offset as _);
    }

    #[inline]
    pub unsafe fn prep_read_fixed(
        &mut self,
        fd: RawFd,
        buf: &mut [u8],
        offset: usize,
        buf_index: usize,
    ) {
        let len = buf.len();
        let addr = buf.as_mut_ptr();
        uring_sys::io_uring_prep_read_fixed(self.sqe,
                                      fd,
                                      addr as _,
                                      len as _,
                                      offset as _,
                                      buf_index as _);
    }

    #[inline]
    pub unsafe fn prep_write_vectored(
        &mut self,
        fd: RawFd,
        bufs: &[io::IoSlice<'_>],
        offset: usize,
    ) {
        let len = bufs.len();
        let addr = bufs.as_ptr();
        uring_sys::io_uring_prep_writev(self.sqe, fd, addr as _, len as _, offset as _);
    }

    #[inline]
    pub unsafe fn prep_write_fixed(
        &mut self,
        fd: RawFd,
        buf: &[u8],
        offset: usize,
        buf_index: usize,
    ) {
        let len = buf.len();
        let addr = buf.as_ptr();
        uring_sys::io_uring_prep_write_fixed(self.sqe,
                                       fd, addr as _,
                                       len as _,
                                       offset as _,
                                       buf_index as _);
    }

    #[inline]
    pub unsafe fn prep_fsync(&mut self, fd: RawFd, flags: FsyncFlags) {
        uring_sys::io_uring_prep_fsync(self.sqe, fd, flags.bits() as _);
    }

    #[inline]
    pub unsafe fn prep_timeout(&mut self, ts: &uring_sys::__kernel_timespec) {
        self.prep_timeout_with_flags(ts, 0, TimeoutFlags::empty());
    }

    #[inline]
    pub unsafe fn prep_timeout_remove(&mut self, user_data: u64) {
        uring_sys::io_uring_prep_timeout_remove(self.sqe, user_data as _, 0)
    }

    #[inline]
    pub unsafe fn prep_timeout_with_flags(
        &mut self,
        ts: &uring_sys::__kernel_timespec,
        count: usize,
        flags: TimeoutFlags,
    ) {
        uring_sys::io_uring_prep_timeout(self.sqe,
                                   ts as *const _ as *mut _,
                                   count as _,
                                   flags.bits() as _);
    }

    #[inline]
    pub unsafe fn prep_sendmsg(&mut self, fd: RawFd, msg: &libc::msghdr, flags: SendMsgFlags) {
        uring_sys::io_uring_prep_sendmsg(self.sqe, fd, msg, flags.bits())
    }

    #[inline]
    pub unsafe fn prep_recvmsg(&mut self, fd: RawFd, msg: &mut libc::msghdr, flags: RecvMsgFlags) {
        uring_sys::io_uring_prep_recvmsg(self.sqe, fd, msg, flags.bits())
    }

    #[inline]
    pub unsafe fn prep_poll_add(&mut self, fd: RawFd, poll_mask: PollMask) {
        uring_sys::io_uring_prep_poll_add(self.sqe, fd, poll_mask.bits())
    }

    #[inline]
    pub unsafe fn prep_poll_remove(&mut self, user_data: u64) {
        uring_sys::io_uring_prep_poll_remove(self.sqe, user_data as *mut _)
    }

    #[inline]
    pub unsafe fn prep_accept(&mut self, fd: RawFd, addr: &mut libc::sockaddr, addr_len: &mut libc::socklen_t, flags: AcceptFlags) {
        uring_sys::io_uring_prep_accept(self.sqe, fd, addr, addr_len, flags.bits())
    }

    #[inline]
    pub unsafe fn io_uring_prep_cancel(&mut self, user_data: u64) {
        uring_sys::io_uring_prep_cancel(self.sqe, user_data as _, 0)
    }

    #[inline]
    pub  unsafe fn prep_link_timeout(&mut self, ts: &mut __kernel_timespec) {
        uring_sys::io_uring_prep_link_timeout(self.sqe, ts, 0)
    }

    #[link_name = "rust_io_uring_prep_connect"]
    pub unsafe fn io_uring_prep_connect(&mut self, fd: RawFd, addr: &mut libc::sockaddr, addr_len: libc::socklen_t) {
        uring_sys::io_uring_prep_connect(self.sqe, fd, addr, addr_len)
    }

    #[inline]
    pub unsafe fn prep_nop(&mut self) {
        uring_sys::io_uring_prep_nop(self.sqe);
    }

    pub fn clear(&mut self) {
        *self.sqe = unsafe { mem::zeroed() };
    }

    pub fn raw(&self) -> &uring_sys::io_uring_sqe {
        &self.sqe
    }

    pub fn raw_mut(&mut self) -> &mut uring_sys::io_uring_sqe {
        &mut self.sqe
    }
}

unsafe impl<'a> Send for SubmissionQueueEvent<'a> { }
unsafe impl<'a> Sync for SubmissionQueueEvent<'a> { }

bitflags::bitflags! {
    pub struct SubmissionFlags: u8 {
        const FIXED_FILE    = 1 << 0;   /* use fixed fileset */
        const IO_DRAIN      = 1 << 1;   /* issue after inflight IO */
        const IO_LINK       = 1 << 2;   /* next IO depends on this one */
    }
}

bitflags::bitflags! {
    pub struct FsyncFlags: u32 {
        const FSYNC_DATASYNC    = 1 << 0;
    }
}

bitflags::bitflags! {
    pub struct TimeoutFlags: u32 {
        const TIMEOUT_ABS   = 1 << 0;
    }
}

bitflags::bitflags! {
    pub struct PollMask: i16 {
        const POLLIN     = libc::POLLIN;
        const POLLPRI    = libc::POLLPRI;
        const POLLOUT    = libc::POLLOUT;
        const POLLERR    = libc::POLLERR;
        const POLLHUP    = libc::POLLHUP;
        const POLLNVAL   = libc::POLLNVAL;
        const POLLRDNORM = libc::POLLRDNORM;
        const POLLRDBAND = libc::POLLRDBAND;
    }
}

bitflags::bitflags! {
    pub struct SendMsgFlags: libc::c_uint {
        const MSG_CONFIRM   = libc::MSG_CONFIRM as _;
        const MSG_DONTROUTE = libc::MSG_DONTROUTE as _;
        const MSG_DONTWAIT  = libc::MSG_DONTWAIT as _;
        const MSG_EOR       = libc::MSG_EOR as _;
        const MSG_MORE      = libc::MSG_MORE as _;
        const MSG_NOSIGNAL  = libc::MSG_NOSIGNAL as _;
        const MSG_OOB       = libc::MSG_OOB as _;
    }
}


bitflags::bitflags! {
    pub struct RecvMsgFlags: libc::c_uint {
        const MSG_CMSG_CLOEXEC = libc::MSG_CMSG_CLOEXEC as _;
        const MSG_DONTWAIT     = libc::MSG_DONTWAIT as _;
        const MSG_ERRQUEUE     = libc::MSG_ERRQUEUE as _;
        const MSG_OOB          = libc::MSG_OOB as _;
        const MSG_PEEK         = libc::MSG_PEEK as _;
        const MSG_TRUNC        = libc::MSG_TRUNC as _;
        const MSG_WAITALL      = libc::MSG_WAITALL as _;
    }
}

bitflags::bitflags! {
    pub struct AcceptFlags: libc::c_int {
        const SOCK_NONBLOCK = libc::SOCK_NONBLOCK;
        const SOCK_CLOEXEC  = libc::SOCK_CLOEXEC;
    }
}