mod dma;
mod mmc;
mod registers;

use axdriver_base::{BaseDriverOps, DeviceType};
use log::{info, warn};

pub use mmc::MMC as DwMshcDriver;

const JH7110_IRQ_NUM: usize = 33;
