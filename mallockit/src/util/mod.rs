#[macro_export]
macro_rules! meta_box {
    ($e: expr) => {
        Box::new_in($e, $crate::space::meta::Meta)
    };
}

#[macro_export]
macro_rules! name_list {
    ($name: ident: $($id: ident),* $(,)*) => {
        #[macro_export]
        macro_rules! $name {
            ($__: ident) => {
                $($__!($id);)*
            };
        }
    };
}

mod address;
#[macro_use]
pub mod aligned_block;
pub mod allocation_area;
pub mod arena;
pub mod bits;
pub mod discrete_tlab;
pub mod freelist;
pub mod heap;
mod layout_utils;
mod lazy;
#[macro_use]
pub mod malloc;
pub mod memory;
mod page;
pub mod size_class;

#[cfg(target_os = "macos")]
pub(crate) mod macos_malloc_zone;

pub use address::*;
pub use core::alloc::Layout;
pub use layout_utils::*;
pub use lazy::*;
pub use page::*;
