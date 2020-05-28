use std::io;

#[test]
fn attach() -> io::Result<()> {
    let ring = iou::IoUring::new(32)?;

    // confirm that attach_wq is working
    let mut init_params = iou::InitParams::default();
    init_params.attach_wq(&ring);
    let mut attached_ring = iou::IoUring::new_with_init_params(32, init_params)?;

    // confirm that submit and enter works on attached ring
    unsafe {
        let mut sq = attached_ring.sq();
        let mut sqe = sq.next_sqe().unwrap();
        sqe.prep_nop();
        sqe.set_user_data(0xDEADBEEF);
    }
    attached_ring.sq().submit()?;

    // confirm that cq reading works on attached ring
    {
        let mut cq = attached_ring.cq();
        let cqe = cq.wait_for_cqe()?;
        assert_eq!(cqe.user_data(), 0xDEADBEEF);
    }

    Ok(())
}