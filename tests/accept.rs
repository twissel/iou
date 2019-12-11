#![feature(test)]
extern crate libc;
extern crate test;

use std::{
    io,
    net::{SocketAddr, TcpListener},
    os::unix::io::AsRawFd,
};

#[test]
fn test_accept() -> io::Result<()> {
    let mut listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;

    let mut ring = iou::IoUring::new(4)?;
    unsafe {
        let mut sqe = ring.next_sqe().unwrap();
        sqe.prep_accept(listener.as_raw_fd(), std::ptr::null_mut(), std::ptr::null_mut(), iou::AcceptFlags::empty());
    }
    Ok(())
}
