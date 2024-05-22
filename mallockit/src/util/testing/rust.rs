use std::{alloc::Allocator, collections::LinkedList};

pub fn simple_boxed(alloc: impl Allocator) {
    let mut v = Box::new_in(42, alloc);
    assert_eq!(*v, 42);
    *v = 23;
    assert_eq!(*v, 23);
}

pub fn simple_vec(alloc: impl Allocator) {
    let mut v = Vec::new_in(alloc);
    v.push(42);
    assert_eq!(v[0], 42);
    v.push(23);
    assert_eq!(v[1], 23);
}

pub fn simple_linked_list(alloc: impl Allocator) {
    let mut list = LinkedList::new_in(alloc);
    list.push_back(42);
    assert_eq!(list.pop_front(), Some(42));
    list.push_back(23);
    assert_eq!(list.pop_front(), Some(23));
    assert_eq!(list.pop_front(), None);
}

#[macro_export]
#[doc(hidden)]
macro_rules! rust_allocator_tests {
    ($global: expr, $name: ident) => {
        #[test]
        fn $name() {
            $crate::util::testing::rust::$name($global);
        }
    };
    ($global: expr) => {
        $crate::rust_allocator_tests!($global, simple_boxed);
        $crate::rust_allocator_tests!($global, simple_vec);
        $crate::rust_allocator_tests!($global, simple_linked_list);
    };
}
