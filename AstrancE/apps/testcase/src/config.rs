pub const HEAP_SIZE: usize = 4 * 1024 * 1024;
pub const USER_SPACE_BASE: usize = 0x10000;
pub const USER_SPACE_SIZE: usize = 32 * 1024;

pub const USER_STACK_TOP: usize = 0x4_0000_0000;
pub const USER_STACK_SIZE: usize = 0x1_0000;

// The size of the kernel stack.
pub const KERNEL_STACK_SIZE: usize = 0x40000;
