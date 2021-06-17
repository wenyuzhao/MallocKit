use std::alloc::Layout;

pub trait LayoutUtils: Sized {
    fn layout(&self) -> Layout;

    #[inline(always)]
    fn padded_size(&self) -> usize {
        let layout = self.layout();
        layout.padding_needed_for(layout.align()) + layout.size()
    }

    #[inline(always)]
    unsafe fn pad_to_align_unchecked(&self) -> Layout {
        let layout = self.layout();
        let pad = layout.padding_needed_for(layout.align());
        let size = layout.size() + pad;
        let align = layout.align();
        debug_assert!(align.is_power_of_two());
        debug_assert!(size <= usize::MAX - (align - 1));
        Layout::from_size_align_unchecked(size, align)
    }

    #[inline(always)]
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

impl const LayoutUtils for Layout {
    #[inline(always)]
    fn layout(&self) -> Layout {
        *self
    }
}
