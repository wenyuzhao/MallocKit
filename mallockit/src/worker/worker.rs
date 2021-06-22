use super::WorkerGroup;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct WorkerId(pub(super) usize);

pub trait Worker: Sized + 'static {
    fn new(id: WorkerId) -> Self;
    fn init(&'static mut self, _group: &'static WorkerGroup<Self>) {}
    fn run(&'static mut self);
}
