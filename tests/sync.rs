use std::io;
use std::sync::Arc;

#[test]
fn noop_test() -> io::Result<()> {
    // confirm that setup and mmap work
    let mut io_uring = Box::leak(Box::new(iou::IoUring::new(32)?));
    let (sq, mut cq, _) = io_uring.queues_sync();
    let sq = Arc::new(sq);
    let sq_clone = sq.clone();
    let h0 = std::thread::spawn(move || {
        unsafe {
            let mut sqe = sq_clone.next_sqe().unwrap();
            sqe.prep_nop();
            sqe.set_user_data(0xDEADBEEF);
        }
        sq_clone.submit()
    });

    let sq_clone = sq.clone();
    let h1 = std::thread::spawn(
        move || {
            unsafe {
                let mut sqe = sq_clone.next_sqe().unwrap();
                sqe.prep_nop();
                sqe.set_user_data(0xDEADBEE1);
            }
            sq_clone.submit()
        }
    );
    h0.join().unwrap();
    h1.join().unwrap();

    // confirm that cq reading works
    {
        let _ = cq.wait_for_cqes(2)?;
    }

    Ok(())
}
