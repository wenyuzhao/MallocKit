mod address;
mod address_non_null;
pub mod bits;
pub mod freelist;
mod lab;
mod lazy;
mod page;

#[cfg(target_os = "macos")]
pub(crate) mod macos_malloc_zone;

pub use address::*;
pub use address_non_null::*;
pub use core::alloc::Layout;
pub use lab::*;
pub use lazy::*;
pub use page::*;

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
