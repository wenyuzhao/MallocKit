use mallockit::util::{Address, Lazy, Local};
use spin::Mutex;

use crate::{
    block::{Block, BlockExt},
    hoard_space::HoardSpace,
};

struct BlockList {
    head: Option<Block>,
    tail: Option<Block>,
    // num_blocks: usize,
    total_bytes: usize,
    used_bytes: usize,
}

impl BlockList {
    const fn new() -> Self {
        Self {
            head: None,
            tail: None,
            // num_blocks: 0,
            total_bytes: 0,
            used_bytes: 0,
        }
    }

    fn add(&mut self, mut block: Block) {
        block.next = self.head;
        self.head = Some(block);
        if self.tail.is_none() {
            self.tail = Some(block)
        }
        self.total_bytes += Block::DATA_BYTES;
    }

    #[inline(always)]
    fn should_flush(&self) -> bool {
        (self.used_bytes * 100 / self.total_bytes) < 10
    }

    fn merge(&mut self, global_pool: &Pool, bin: Self) {
        {
            let mut b = bin.head;
            while let Some(mut x) = b {
                x.owner = Some(global_pool.static_ref());
                b = x.next;
            }
        }
        if self.tail.is_none() {
            self.head = bin.head;
            self.tail = bin.tail;
        } else {
            self.tail.unwrap().next = bin.head;
        }
        self.total_bytes += bin.total_bytes;
        self.used_bytes += bin.used_bytes;
    }
}

pub struct Pool {
    pub global: bool,
    // pub space: Option<Lazy<&'static HoardSpace, Local>>,
    blocks: Mutex<[BlockList; 9]>,
}

impl Pool {
    pub const fn new(global: bool) -> Self {
        const fn b() -> BlockList {
            BlockList::new()
        }
        Self {
            global,
            blocks: Mutex::new([b(), b(), b(), b(), b(), b(), b(), b(), b()]),
        }
    }

    pub const fn static_ref(&self) -> &'static Self {
        unsafe { &*(self as *const Self) }
    }

    pub fn pop_block(&self, size_class: usize) -> Option<Block> {
        debug_assert!(self.global);
        let mut blocks = self.blocks.lock();
        if let Some(block) = blocks[size_class].head {
            let next = block.next;
            blocks[size_class].head = next;
            if next.is_none() {
                blocks[size_class].tail = None;
            }
            return Some(block);
        }
        None
    }

    fn flush(&self, size_class: usize, list: BlockList) {
        debug_assert!(self.global);
        let mut blocks = self.blocks.lock();
        blocks[size_class].merge(self, list);
    }

    #[inline(always)]
    pub fn alloc_cell(
        &self,
        size_class: usize,
        space: &Lazy<&'static HoardSpace, Local>,
    ) -> Option<Address> {
        // println!("alloc_cell {:?}", size_class);
        let mut blocks = self.blocks.lock();
        // Get a local block
        let block = {
            // Go through the list to find a non-full block
            let mut target = None;
            let mut block = blocks[size_class].head;
            while let Some(b) = block {
                if b.head_cell.is_some() {
                    target = Some(b);
                    break;
                }
                block = b.next;
            }
            // Quit if not found
            // if target.is_none() {
            //     println!("alloc_cell failed");
            //     return None;
            // }
            // println!("alloc_cell {:?} target {:?}", size_class, target);
            match target {
                Some(block) => block,
                _ => {
                    // Get a block from global pool
                    let block = space.acquire_block(self, size_class).unwrap();
                    debug_assert!(block.head_cell.is_some());
                    blocks[size_class].add(block);
                    block
                }
            }
        };
        // println!("alloc_cell {:?} {:?}", size_class, block);
        // println!("{:?}", block);
        // Alloc a cell from the block
        let cell = block.alloc_cell().unwrap();
        blocks[size_class].used_bytes += HoardSpace::size_class_to_bytes(size_class);
        Some(cell)
    }

    #[inline(always)]
    pub fn free_cell(&self, cell: Address, space: &Lazy<&'static HoardSpace, Local>) {
        let mut blocks = self.blocks.lock();
        let block = Block::containing(cell);
        let size_class = block.size_class;
        block.free_cell(cell);
        blocks[size_class].used_bytes -= HoardSpace::size_class_to_bytes(size_class);
        if !self.global && blocks[size_class].should_flush() {
            debug_assert!(!self.global);
            let mut list = BlockList::new();
            std::mem::swap(&mut list, &mut blocks[size_class]);
            space.pool.flush(size_class, list);
        }
    }
}
