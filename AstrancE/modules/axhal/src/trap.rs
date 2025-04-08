//! Trap handling.

use linkme::distributed_slice as def_trap_handler;
use memory_addr::VirtAddr;
use page_table_entry::MappingFlags;

#[cfg(feature = "uspace")]
use crate::arch::TrapFrame;

pub use linkme::distributed_slice as register_trap_handler;

/// A slice of IRQ handler functions.
#[def_trap_handler]
pub static IRQ: [fn(usize) -> bool];

/// A slice of page fault handler functions.
#[def_trap_handler]
pub static PAGE_FAULT: [fn(VirtAddr, MappingFlags, bool) -> bool];

/// A slice of syscall handler functions.
#[cfg(feature = "uspace")]
#[def_trap_handler]
pub static SYSCALL: [fn(&TrapFrame, usize) -> Option<isize>];

#[allow(unused_macros)]
macro_rules! handle_trap {
    ($trap:ident, $($args:tt)*) => {{
        let mut iter = $crate::trap::$trap.iter();
        if let Some(func) = iter.next() {
            if iter.next().is_some() {
                warn!("Multiple handlers for trap {} are not currently supported", stringify!($trap));
            }
            func($($args)*)
        } else {
            warn!("No registered handler for trap {}", stringify!($trap));
            false
        }
    }}
}

/// Call the external syscall handler.
/// Handlers can overlap each other but only one of them can return Some(isize).
#[cfg(feature = "uspace")]
pub(crate) fn handle_syscall(tf: &TrapFrame, syscall_num: usize) -> isize {
    let mut result = None;
    let mut result_count: usize = 0;
    let args = [
        tf.arg0(),
        tf.arg1(),
        tf.arg2(),
        tf.arg3(),
        tf.arg4(),
        tf.arg5(),
    ];

    debug!("handling syscall: {} with args: {:?}", syscall_num, args);
    for handler in SYSCALL {
        if let Some(r) = handler(tf, syscall_num) {
            if result_count > 1 {
                panic!("Multiple syscall handlers returned a value");
            }
            result = Some(r);
            result_count += 1;
        }
    }

    debug!("syscall_handler result: {:?}", result);
    result.expect("None of syscall handlers returned a value")
}
