#![allow(missing_docs)]

//#[cfg(feature = "loongarch64")]
#[macro_use]
mod loongarch;

pub use loongarch::*;

//#[cfg(target_pointer_width = "64")]
macro_rules! save {
    ($reg:ident => $ptr:ident[$pos:expr]) => {
        concat!(
            "st.d ",
            stringify!($reg),
            ", 8*",
            $pos,
            '(',
            stringify!($ptr),
            ')'
        )
    };
}

macro_rules! load {
    ($ptr:ident[$pos:expr] => $reg:ident) => {
        concat!(
            "ld.d ",
            stringify!($reg),
            ", 8*",
            $pos,
            '(',
            stringify!($ptr),
            ')'
        )
    };
}

use crate::TrapHandler;
use core::alloc::Layout;

#[repr(C)]
#[allow(missing_docs)]
pub struct FlowContext {
    pub ra: usize,      // 0..
    pub t: [usize; 7],  // 1.. (对应龙芯的$r12-$r18)
    pub a: [usize; 8],  // 8.. (对应龙芯的$r4-$r11)
    pub s: [usize; 12], // 16.. (对应龙芯的$r22-$r31)
    pub gp: usize,      // 28.. (对应龙芯的$r21)
    pub tp: usize,      // 29.. (对应龙芯的$r20)
    pub sp: usize,      // 30..
    pub pc: usize,      // 31..
}

impl FlowContext {
    /// 零初始化
    pub const ZERO: Self = Self {
        ra: 0,
        t: [0; 7],
        a: [0; 8],
        s: [0; 12],
        gp: 0,
        tp: 0,
        sp: 0,
        pc: 0,
    };
}

/// 复用当前栈为陷入栈
#[naked]
pub unsafe extern "C" fn reuse_stack_for_trap() {
    const LAYOUT: Layout = Layout::new::<TrapHandler>();
    core::arch::naked_asm!(
        "   addi.d $sp, $sp, {size}
            andi $sp, $sp, {mask}
            jirl $zero, $ra, 0
        ",
        size = const -(LAYOUT.size() as isize),
        mask = const !(LAYOUT.align() as isize - 1),
    )
}

/// 陷入处理例程
#[naked]
pub unsafe extern "C" fn trap_entry() {

    core::arch::naked_asm!(
        ".align 4",
        // 换栈
        exchange!(),
        // 加载上下文指针
        save!(a0 => sp[2]),
        load!(sp[0] => a0),
        // 保存寄存器
        save!(ra => a0[0]),
        save!(t0 => a0[1]),
        save!(t1 => a0[2]),
        save!(t2 => a0[3]),
        save!(t3 => a0[4]),
        save!(t4 => a0[5]),
        save!(t5 => a0[6]),
        save!(t6 => a0[7]),
        // 调用处理函数
        "move $a0, $sp",
        load!(sp[1] => ra),
        "jirl $ra, $ra, 0",
        // 恢复处理
        "0:",
        load!(sp[0] => a1),
        "beqz $a0, 0f",
        "addi.d $a0, $a0, -1",
        "beqz $a0, 1f",
        "addi.d $a0, $a0, -1",
        "beqz $a0, 2f",
        "addi.d $a0, $a0, -1",
        "beqz $a0, 3f",
        // 完整路径保存
        save!(s0 => a1[16]),
        save!(s1 => a1[17]),
        save!(s2 => a1[18]),
        save!(s3 => a1[19]),
        save!(s4 => a1[20]),
        save!(s5 => a1[21]),
        save!(s6 => a1[22]),
        save!(s7 => a1[23]),
        save!(s8 => a1[24]),
        save!(s9 => a1[25]),
        save!(s10 => a1[26]),
        save!(s11 => a1[27]),
        // 调用完整处理
        "move $a0, $sp",
        load!(sp[2] => ra),
        "jirl $ra, $ra, 0",
        "b 0b",
        // 恢复寄存器
        "3:",
        load!(a1[16] => s0),
        load!(a1[17] => s1),
        load!(a1[18] => s2),
        load!(a1[19] => s3),
        load!(a1[20] => s4),
        load!(a1[21] => s5),
        load!(a1[22] => s6),
        load!(a1[23] => s7),
        load!(a1[24] => s8),
        load!(a1[25] => s9),
        load!(a1[26] => s10),
        load!(a1[27] => s11),
        "2:",
        load!(a1[0] => ra),
        load!(a1[1] => t0),
        load!(a1[2] => t1),
        load!(a1[3] => t2),
        load!(a1[4] => t3),
        load!(a1[5] => t4),
        load!(a1[6] => t5),
        load!(a1[7] => t6),
        "1:",
        load!(a1[10] => a2),
        load!(a1[11] => a3),
        load!(a1[12] => a4),
        load!(a1[13] => a5),
        load!(a1[14] => a6),
        load!(a1[15] => a7),
        "0:",
        load!(a1[8] => a0),
        load!(a1[9] => a1),
        exchange!(),
        r#return!(),
    )
}
