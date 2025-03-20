use super::*;

#[test]
fn write() {
    assert_eq!(
        syscall_handler(64, [1, "addr to buffer".as_ptr() as usize, 9,0,0,0]),Ok(9));
}

