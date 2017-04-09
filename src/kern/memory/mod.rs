pub mod paging;
pub mod frame;
pub mod inactive;
pub mod mapper;

pub use frame::*;

pub const PAGE_SIZE: usize = 4096;
