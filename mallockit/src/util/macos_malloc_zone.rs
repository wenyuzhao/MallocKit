use super::Address;

const ZONE_NAME: &'static [i8] = &[
    'm' as _, 'a' as _, 'l' as _, 'l' as _, 'o' as _, 'c' as _, 'k' as _, 'i' as _, 't' as _, 0,
];

pub static mut MALLOCKIT_MALLOC_ZONE: MallocZone = MallocZone {
    reserved: [0; 2],
    size: zone_size,
    malloc: zone_malloc,
    calloc: zone_calloc,
    valloc: zone_valloc,
    free: zone_free,
    realloc: zone_realloc,
    destroy: zone_destroy,
    zone_name: ZONE_NAME.as_ptr(),
    /// Nullable
    batch_malloc: 0 as _,
    /// Nullable
    batch_free: 0 as _,
    introspect: unsafe { &MALLOCKIT_MALLOC_INTROSPECTION },
    version: 9,
    memalign: zone_memalign,
    free_definite_size: 0 as _,
    pressure_relief: 0 as _,
    claimed_address: 0 as _,
};

static mut MALLOCKIT_MALLOC_INTROSPECTION: MallocIntrospection = MallocIntrospection {
    enumerator: intro_enumerator,
    good_size: zone_size,
    check: intro_check,
    print: intro_print,
    log: intro_log,
    force_lock: intro_force_lock,
    force_unlock: intro_force_unlock,
    statistics: intro_statistics,
    zone_locked: intro_zone_locked,
    enable_discharge_checking: 0 as _,
    disable_discharge_checking: 0 as _,
    discharge: 0 as _,
    reinit_lock: 0 as _,
    print_task: 0 as _,
    task_statistics: 0 as _,
};

extern "C" {
    fn valloc(size: usize) -> *mut u8;
    fn malloc_size(ptr: *mut u8) -> usize;
}

unsafe extern "C" fn zone_size(_: *const MallocZone, ptr: Address) -> usize {
    malloc_size(ptr.into())
}

unsafe extern "C" fn zone_malloc(_: *mut MallocZone, size: usize) -> Address {
    libc::malloc(size).into()
}

unsafe extern "C" fn zone_calloc(_: *mut MallocZone, num_items: usize, size: usize) -> Address {
    libc::calloc(num_items, size).into()
}

unsafe extern "C" fn zone_valloc(_: *mut MallocZone, size: usize) -> Address {
    valloc(size).into()
}

unsafe extern "C" fn zone_free(_: *mut MallocZone, p: Address) {
    libc::free(p.into())
}

unsafe extern "C" fn zone_realloc(_: *mut MallocZone, p: Address, size: usize) -> Address {
    libc::realloc(p.into(), size).into()
}

unsafe extern "C" fn zone_memalign(_: *mut MallocZone, align: usize, size: usize) -> Address {
    let mut p = 0 as _;
    libc::posix_memalign(&mut p, align, size);
    p.into()
}

unsafe extern "C" fn zone_destroy(_: *mut MallocZone) {}

unsafe extern "C" fn intro_enumerator(
    _: u32,
    _: usize,
    _: u32,
    _: Address,
    _: MemoryReader,
    _: VMRangeRecorder,
) -> i32 {
    0
}

unsafe extern "C" fn intro_zone_locked(_: *mut MallocZone) -> u32 {
    0
}
unsafe extern "C" fn intro_check(_: *mut MallocZone) -> u32 {
    1
}
unsafe extern "C" fn intro_print(_: *mut MallocZone, _: u32) {}
unsafe extern "C" fn intro_log(_: *mut MallocZone, _: Address) {}

unsafe extern "C" fn intro_force_lock(_: *mut MallocZone) {}
unsafe extern "C" fn intro_force_unlock(_: *mut MallocZone) {}

unsafe extern "C" fn intro_statistics(_: *mut MallocZone, stats: *mut MallocStatistics) {
    *stats = Default::default();
}

#[repr(C)]
pub struct MallocZone {
    reserved: [usize; 2],
    size: unsafe extern "C" fn(zone: *const MallocZone, ptr: Address) -> usize,
    malloc: unsafe extern "C" fn(zone: *mut MallocZone, size: usize) -> Address,
    calloc: unsafe extern "C" fn(zone: *mut MallocZone, num_items: usize, size: usize) -> Address,
    valloc: unsafe extern "C" fn(zone: *mut MallocZone, size: usize) -> Address,
    free: unsafe extern "C" fn(zone: *mut MallocZone, ptr: Address),
    realloc: unsafe extern "C" fn(zone: *mut MallocZone, ptr: Address, size: usize) -> Address,
    destroy: unsafe extern "C" fn(zone: *mut MallocZone),
    zone_name: *const libc::c_char,
    /// Nullable
    batch_malloc: *const unsafe extern "C" fn(
        zone: *mut MallocZone,
        size: usize,
        results: *mut Address,
        num_requested: usize,
    ) -> usize,
    /// Nullable
    batch_free: *const unsafe extern "C" fn(
        zone: *mut MallocZone,
        to_be_freed: *mut Address,
        num_to_be_freed: usize,
    ),
    introspect: *const MallocIntrospection,
    version: u32,
    /// Nullable
    memalign: unsafe extern "C" fn(zone: *mut MallocZone, align: usize, size: usize) -> Address,
    /// Nullable
    free_definite_size:
        *const unsafe extern "C" fn(zone: *mut MallocZone, ptr: Address, size: usize) -> Address,
    /// Nullable
    pressure_relief: *const unsafe extern "C" fn(zone: *mut MallocZone, goal: usize) -> Address,
    /// Nullable
    claimed_address: *const unsafe extern "C" fn(zone: *mut MallocZone, ptr: Address) -> u32,
}

type MemoryReader = extern "C" fn(
    remote_task: u32,
    remote_address: Address,
    size: usize,
    local_memory: *const Address,
) -> i32;

type VMRangeRecorder = extern "C" fn(u32, Address, u32, Address, u32);

#[derive(Default)]
#[allow(unused)]
#[repr(C)]
struct MallocStatistics {
    blocks_in_use: u32,
    size_in_use: usize,
    /// high water mark of touched memory
    max_size_in_use: usize,
    /// reserved in memory
    size_allocated: usize,
}

#[allow(unused)]
#[repr(C)]
struct MallocIntrospection {
    /// Enumerates all the malloc pointers in use
    enumerator: unsafe extern "C" fn(
        task: u32,
        _: usize,
        type_mask: u32,
        zone_address: Address,
        reader: MemoryReader,
        recorder: VMRangeRecorder,
    ) -> i32,
    good_size: unsafe extern "C" fn(zone: *const MallocZone, ptr: Address) -> usize,
    /// Consistency checker
    check: unsafe extern "C" fn(zone: *mut MallocZone) -> u32,
    /// Prints zone
    print: unsafe extern "C" fn(zone: *mut MallocZone, verbose: u32),
    /// Enables logging of activity
    log: unsafe extern "C" fn(zone: *mut MallocZone, address: Address),
    /// Forces locking zone
    force_lock: unsafe extern "C" fn(zone: *mut MallocZone),
    /// Forces unlocking zone
    force_unlock: unsafe extern "C" fn(zone: *mut MallocZone),
    /// Fills statistics
    statistics: unsafe extern "C" fn(zone: *mut MallocZone, stats: *mut MallocStatistics),
    /// Are any zone locks held
    zone_locked: unsafe extern "C" fn(zone: *mut MallocZone) -> u32,
    /// Discharge checking. Present in version >= 7.
    enable_discharge_checking: *const unsafe extern "C" fn(zone: *mut MallocZone) -> u32,
    /// Discharge checking. Present in version >= 7.
    disable_discharge_checking: *const unsafe extern "C" fn(zone: *mut MallocZone),
    /// Discharge checking. Present in version >= 7.
    discharge: *const unsafe extern "C" fn(zone: *mut MallocZone, memory: Address),
    /// Reinitialize zone locks, called only from atfork_child handler. Present in version >= 9.
    reinit_lock: *const unsafe extern "C" fn(zone: *mut MallocZone),
    /// Debug print for another process. Present in version >= 11.
    print_task: *const unsafe extern "C" fn(
        task: u32,
        level: u32,
        zone_address: Address,
        reader: MemoryReader,
        printer: !,
    ),
    /// Present in version >= 12
    task_statistics: *const unsafe extern "C" fn(
        task: u32,
        zone_address: Address,
        reader: MemoryReader,
        stats: *mut MallocStatistics,
    ),
}

extern "C" {
    #[allow(unused)]
    fn malloc_zone_register(_: *const MallocZone);
    fn malloc_zone_from_ptr(_: Address) -> *const MallocZone;
}

pub fn external_memory_size(ptr: Address) -> usize {
    unsafe {
        let zone = malloc_zone_from_ptr(ptr);
        ((*zone).size)(zone, ptr)
    }
}

pub fn init() {
    #[cfg(feature = "macos_malloc_zone_override")]
    unsafe {
        malloc_zone_register(&MALLOCKIT_MALLOC_ZONE);
    }
}
