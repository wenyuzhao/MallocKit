use mallockit_rs::MallocKit;

#[global_allocator]
static MALLOC_KIT: MallocKit = MallocKit;

fn main() {
    println!("Hello, world!");
    let v = vec![1, 2, 3];
    let s = v
        .iter()
        .map(|x| format!("{x}"))
        .collect::<Vec<_>>()
        .join(", ");
    println!("vec: {s}");
}
