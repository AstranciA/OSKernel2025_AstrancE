use super::{FlowContext, trap_entry};
use core::arch::asm;

macro_rules! exchange {
    () => {
        exchange!(sp)
    };
    ($reg:ident) => {
        concat!("csrwr ", stringify!($reg), ", 0x30, ", stringify!($reg)) // LOONGARCH_CSR_KS0
    };
}
macro_rules! r#return {
    () => {
        "ertn"
    };
}

/// 龙芯架构专用控制状态寄存器
mod csr {
    pub const ECFG: usize = 0x4;    // 异常配置寄存器
    pub const ESTAT: usize = 0x5;   // 异常状态寄存器
    pub const ERA: usize = 0x6;     // 异常返回地址
    pub const EENTRY: usize = 0xc;  // 异常入口基址
    pub const SAVE0: usize = 0x30;  // 临时存储寄存器
}

impl FlowContext {
    /// 加载非调用规范约定的寄存器
    #[inline]
    pub(crate) unsafe fn load_others(&self) {
        unsafe {
            asm!(
                "   move $gp, {gp}
                move $tp, {tp}
                csrwr {sp}, {save}
                csrwr {pc}, {era}
            ",
                gp = in(reg) self.gp,
                tp = in(reg) self.tp,
                sp = in(reg) self.sp,
                pc = in(reg) self.pc,
                save = const csr::SAVE0,
                era = const csr::ERA,
            );
        }
    }
}

/// 交换临时存储寄存器
#[inline]
pub(crate) fn exchange_scratch(mut val: usize) -> usize {
    unsafe { asm!("csrwr {0}, {1}", inlateout(reg) val, const csr::SAVE0) };
    val
}

/// 设置全局陷入入口
#[inline]
pub unsafe fn load_direct_trap_entry() {
    unsafe { 
        asm!(
            "csrwr {0}, {1}",
            in(reg) trap_entry,
            const csr::EENTRY,
            options(nomem)
        )
    }
}

/// 模拟陷入
#[inline]
pub unsafe fn soft_trap(cause: usize) {
    unsafe {
        asm!(
            "   la.abs $a0, 1f
            csrwr $a0, {era}
            csrwr {cause}, {estat}
            b {trap}
         1:
        ",
            era = const csr::ERA,
            estat = const csr::ESTAT,
            cause = in(reg) cause,
            trap = sym trap_entry,
            out("a0") _,
        );
    }
}


