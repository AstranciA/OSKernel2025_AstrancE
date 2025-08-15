//! Architecture-specific types and operations.

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        mod x86_64;
        pub use self::x86_64::*;
    } else if #[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))] {
        mod riscv;
        pub use self::riscv::*;
    } else if #[cfg(target_arch = "aarch64")]{
        mod aarch64;
        pub use self::aarch64::*;
    } else if #[cfg(any(target_arch = "loongarch64"))] {
        mod loongarch64;
        pub use self::loongarch64::*;
    }
}

/*
 *trait GeneralTrapFrame {
 *    /// Gets the 0th syscall argument.
 *    pub const fn arg0(&self) -> usize;
 *
 *    /// Gets the 1st syscall argument.
 *    pub const fn arg1(&self) -> usize;
 *
 *    /// Gets the 2nd syscall argument.
 *    pub const fn arg2(&self) -> usize;
 *
 *    /// Gets the 3rd syscall argument.
 *    pub const fn arg3(&self) -> usize;
 *
 *    /// Gets the 4th syscall argument.
 *    pub const fn arg4(&self) -> usize;
 *
 *    /// Gets the 5th syscall argument.
 *    pub const fn arg5(&self) -> usize;
 *
 *    pub fn get_user_sp(&self) -> usize;
 *
 *    /// sepc in riscv64
 *    /// era in loongarch64
 *    pub fn inc_sepc(&mut self);
 *}
 */
