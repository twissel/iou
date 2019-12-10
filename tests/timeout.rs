#![feature(test)]
extern crate libc;
extern crate test;
extern crate uring_sys;

use std::io;
const TIMEOUT_DATA: u64 = 1;
const CANCEL_TIMEOUT_DATA: u64 = 2;

fn next_sqe(ring: &mut iou::IoUring) -> io::Result<iou::SubmissionQueueEvent> {
    ring.next_sqe()
        .ok_or(io::Error::new(io::ErrorKind::Other, "no sqe"))
}

fn prepare_and_submit_timeout(ring: &mut iou::IoUring) -> io::Result<()> {
    let mut sqe = next_sqe(ring)?;
    let ts = uring_sys::__kernel_timespec {
        tv_sec: 1,
        tv_nsec: 0,
    };

    unsafe {
        sqe.prep_timeout(&ts);
        sqe.set_user_data(TIMEOUT_DATA);
        ring.sq().submit()?;
    }
    Ok(())
}

#[test]
fn test_timeout() -> io::Result<()> {
    let mut io_uring = iou::IoUring::new(1)?;
    prepare_and_submit_timeout(&mut io_uring)?;
    let mut cq = io_uring.cq();
    let cqe = cq.wait_for_cqe()?;
    assert_eq!(cqe.user_data(), TIMEOUT_DATA);
    let res = cqe.result();
    match res.err().and_then(|e| e.raw_os_error()) {
        Some(res) => {
            assert_eq!(res, libc::ETIME);
            Ok(())
        }
        None => Err(io::Error::new(
            io::ErrorKind::Other,
            "expected libc::ETIME error, got OK",
        )),
    }
}

#[test]
fn test_timeout_cancel() -> io::Result<()> {
    let mut io_uring = iou::IoUring::new(2)?;
    prepare_and_submit_timeout(&mut io_uring)?;
    let mut sqe = next_sqe(&mut io_uring)?;
    unsafe {
        sqe.prep_timeout_remove(TIMEOUT_DATA);
        sqe.set_user_data(CANCEL_TIMEOUT_DATA);
        io_uring.sq().submit()?;
    }
    let mut cq = io_uring.cq();
    for _ in 0..2 {
        let cqe = cq.wait_for_cqe()?;
        let user_data = cqe.user_data();
        let res = cqe.result();
        if user_data == TIMEOUT_DATA {
            if let Err(e) = res {
                let code = e.raw_os_error().expect("unexpected error");
                if code != libc::ECANCELED {
                    return Err(io::Error::from_raw_os_error(code));
                }
            }
        } else if user_data == CANCEL_TIMEOUT_DATA {
            if let Err(e) = res {
                let code = e.raw_os_error().expect("unexpected error");
                if code == libc::EINVAL {
                    return Ok(());
                } else {
                    return Err(io::Error::from_raw_os_error(code));
                }
            }
        }
    }
    Ok(())
}
