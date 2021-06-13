mod address;
mod address_non_null;
pub mod bits;
pub mod freelist;
mod lab;
mod lazy;
mod page;
pub(crate) mod sys_alloc;

pub use address::*;
pub use address_non_null::*;
pub use core::alloc::Layout;
pub use lab::*;
pub use lazy::*;
pub use page::*;
pub use sys_alloc::*;

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
