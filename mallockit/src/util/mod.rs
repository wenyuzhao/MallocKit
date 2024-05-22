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

pub mod bits;
mod lazy;
#[macro_use]
pub mod malloc;
#[macro_use]
pub mod mem;
#[macro_use]
pub mod sys;
pub mod testing;

#[cfg(target_os = "macos")]
pub(crate) mod macos_malloc_zone;

pub use core::alloc::Layout;
pub use lazy::*;
pub use mem::address::*;
pub use mem::layout_utils::*;
pub use mem::page::*;
