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
pub mod aligned_block;
pub mod arena;
pub mod bits;
pub mod freelist;
mod lab;
mod layout_utils;
mod lazy;
pub mod memory;
pub mod memory_chunk;
mod page;

#[cfg(target_os = "macos")]
pub(crate) mod macos_malloc_zone;

pub use address::*;
pub use core::alloc::Layout;
pub use lab::*;
pub use layout_utils::*;
pub use lazy::*;
pub use page::*;
