/****************************************************************************************************************************************************************************************************************************************************************************
 *                                                                                                                              DOCUMENTATION                                                                                                                               *
 * SIMPLE MODULE USED TO STORE GLOBAL VARIABLES THAT ARE USED ACROSS THE KERNEL, SUCH AS THE KERNEL CODE AND DATA SEGMENT SELECTORS, AND THE IDLE TICKS COUNTER. THESE VARIABLES ARE MUTABLE AND CAN BE ACCESSED AND MODIFIED FROM DIFFERENT PARTS OF THE KERNEL AS NEEDED. *
 ****************************************************************************************************************************************************************************************************************************************************************************/
use core::sync::atomic::AtomicU64;

pub static mut KERNEL_CODE_SEGMENT: u16 = 0;
pub static mut KERNEL_DATA_SEGMENT: u16 = 0;

pub static IDLE_TICKS: AtomicU64 = AtomicU64::new(0);
