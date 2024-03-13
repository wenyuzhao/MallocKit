use std::alloc::Layout;

pub trait LayoutUtils: Sized {
    fn layout(&self) -> Layout;

    fn padded_size(&self) -> usize {
        let layout = self.layout();
        layout.padding_needed_for(layout.align()) + layout.size()
    }

    /// Pad the layout size to multiple of its alignment.
    ///
    /// # Safety
    ///
    /// This function is unsafe as it does not verify the preconditions from Layout::from_size_align.
    unsafe fn pad_to_align_unchecked(&self) -> Layout {
        let layout = self.layout();
        let pad = layout.padding_needed_for(layout.align());
        let size = layout.size() + pad;
        let align = layout.align();
        debug_assert!(align.is_power_of_two());
        debug_assert!(size <= usize::MAX - (align - 1));
        Layout::from_size_align_unchecked(size, align)
    }

    /// Extend the layout with another layout.
    ///
    /// # Safety
    ///
    /// This function is unsafe as it does not verify the preconditions from Layout::from_size_align.
    unsafe fn extend_unchecked(&self, next: Layout) -> (Layout, usize) {
        let layout = self.layout();
        let new_align = usize::max(layout.align(), next.align());
        let pad = layout.padding_needed_for(next.align());
        let size = layout.size();
        debug_assert!(size <= usize::MAX - pad);
        let offset = size + pad;
        debug_assert!(offset <= usize::MAX - next.size());
        let new_size = offset + next.size();
        let layout = Layout::from_size_align_unchecked(new_size, new_align);
        (layout, offset)
    }
}

impl LayoutUtils for Layout {
    fn layout(&self) -> Layout {
        *self
    }

    fn padded_size(&self) -> usize {
        let layout = self.layout();
        layout.padding_needed_for(layout.align()) + layout.size()
    }

    unsafe fn pad_to_align_unchecked(&self) -> Layout {
        let layout = self.layout();
        let pad = layout.padding_needed_for(layout.align());
        let size = layout.size() + pad;
        let align = layout.align();
        debug_assert!(align.is_power_of_two());
        debug_assert!(size <= usize::MAX - (align - 1));
        Layout::from_size_align_unchecked(size, align)
    }

    unsafe fn extend_unchecked(&self, next: Layout) -> (Layout, usize) {
        let layout = self.layout();
        let new_align = if layout.align() > next.align() {
            layout.align()
        } else {
            next.align()
        };
        let pad = layout.padding_needed_for(next.align());
        let size = layout.size();
        debug_assert!(size <= usize::MAX - pad);
        let offset = size + pad;
        debug_assert!(offset <= usize::MAX - next.size());
        let new_size = offset + next.size();
        let layout = Layout::from_size_align_unchecked(new_size, new_align);
        (layout, offset)
    }
}
