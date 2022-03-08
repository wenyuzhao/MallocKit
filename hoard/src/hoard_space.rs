use super::{page_resource::BlockPageResource, Allocator, Space, SpaceId};
use crate::block::{Block, BlockExt};
use mallockit::util::*;
use spin::Mutex;

/// Global heap
pub struct HoardSpace {
    id: SpaceId,
    pr: BlockPageResource,
}

impl Space for HoardSpace {
    const MAX_ALLOCATION_SIZE: usize = Block::BYTES / 2;
    type PR = BlockPageResource;

    fn new(id: SpaceId) -> Self {
        Self {
            id,
            pr: BlockPageResource::new(id, Block::LOG_BYTES),
        }
    }

    #[inline(always)]
    fn id(&self) -> SpaceId {
        self.id
    }

    #[inline(always)]
    fn page_resource(&self) -> &Self::PR {
        &self.pr
    }

    #[inline(always)]
    fn get_layout(ptr: Address) -> Layout {
        let block = Block::containing(ptr);
        let size = 1usize << (block.size_class + 3);
        Layout::from_size_align(size, size).unwrap()
    }
}

impl HoardSpace {
    #[inline(always)]
    pub fn can_allocate(layout: Layout) -> bool {
        let layout = unsafe { layout.pad_to_align_unchecked() };
        let size = layout.size().next_power_of_two();
        size <= Self::MAX_ALLOCATION_SIZE
    }

    #[inline(always)]
    pub fn size_class_of_layout(layout: Layout) -> usize {
        let layout = unsafe { layout.pad_to_align_unchecked() };
        let size = layout.size().next_power_of_two();
        Self::size_class(size)
    }

    #[inline(always)]
    pub const fn size_class(bytes: usize) -> usize {
        debug_assert!(bytes <= Self::MAX_ALLOCATION_SIZE);
        bytes.trailing_zeros() as usize - 3
    }

    #[inline(always)]
    fn acquire_block(&self, local: &HoardLocal, sc: usize) -> Option<Block> {
        let addr = self
            .acquire::<Size4K>(1 << (Block::LOG_BYTES - Size4K::LOG_BYTES))?
            .start
            .start();
        let block = Block::new(addr);
        block.init(unsafe { &*(local as *const HoardLocal) }, sc);
        Some(block)
    }
}

pub struct HoardLocal {
    pub space: Lazy<&'static HoardSpace, Local>,
    pub blocks: Mutex<[Option<Block>; 9]>,
}

impl HoardLocal {
    pub const fn new(space: Lazy<&'static HoardSpace, Local>) -> Self {
        Self {
            space,
            blocks: Mutex::new([None; 9]),
        }
    }

    #[inline(always)]
    fn alloc_cell(&self, sc: usize) -> Option<Address> {
        let mut blocks = self.blocks.lock();
        // Get a local block
        let block = {
            let mut block = blocks[sc];
            while let Some(b) = block {
                if b.head_cell.is_some() {
                    break;
                }
                block = b.next;
            }
            match block {
                Some(b) => b,
                _ => {
                    let mut b = self.space.acquire_block(self, sc)?;
                    b.next = blocks[sc];
                    blocks[sc] = Some(b);
                    b
                }
            }
        };
        block.alloc_cell()
    }

    #[inline(always)]
    fn free_cell(&self, cell: Address) {
        let _blocks = self.blocks.lock();
        let block = Block::containing(cell);
        block.free_cell(cell)
    }
}

/// Thread-local heap
pub struct HoardAllocator {
    local: HoardLocal,
}

impl HoardAllocator {
    pub const fn new(space: Lazy<&'static HoardSpace, Local>, _space_id: SpaceId) -> Self {
        Self {
            local: HoardLocal::new(space),
        }
    }
}

impl Allocator for HoardAllocator {
    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let size_class = HoardSpace::size_class_of_layout(layout);
        self.local.alloc_cell(size_class)
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        let block = Block::containing(ptr);
        block.owner.unwrap().free_cell(ptr);
        // TODO: transfer to global space if the block is mostly empty
    }
}
