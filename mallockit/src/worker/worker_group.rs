use std::sync::{Barrier, Condvar, Mutex};

use crate::space::meta::Meta;

use super::{Worker, WorkerId};

pub struct WorkerGroup<W: Worker> {
    workers: Vec<W, Meta>,
    pub barrier: Barrier,
    monitor: (Condvar, Mutex<()>),
}

impl<W: Worker> WorkerGroup<W> {
    pub fn new(num_workers: usize) -> Self {
        let mut workers = Vec::with_capacity_in(num_workers, Meta);
        for i in 0..num_workers {
            workers.push(W::new(WorkerId(i)));
        }
        Self {
            workers,
            barrier: Barrier::new(num_workers),
            monitor: (Condvar::new(), Mutex::new(())),
        }
    }

    pub fn notify_all(&self) {
        self.monitor.0.notify_all()
    }

    pub fn notify_one(&self) {
        self.monitor.0.notify_one()
    }

    pub fn wait(&self, _: &mut W) {
        let should_wake_up = self.monitor.1.lock().unwrap();
        let _guard = self.monitor.0.wait(should_wake_up).unwrap();
    }

    fn spawn_one(&'static self, ctx: &'static W) {
        let ctx = ctx as *const W as *mut W;
        unsafe {
            (*ctx).init(self);
        }
        extern "C" fn run<W: Worker>(ctx: *mut libc::c_void) -> *mut libc::c_void {
            let ctx = unsafe { &mut *(ctx as *mut W) };
            ctx.run();
            0 as _
        }
        let mut pthread = 0;
        unsafe {
            libc::pthread_create(&mut pthread, 0 as _, run::<W>, ctx as _);
        }
    }

    pub fn spawn(&'static self) {
        for i in 0..self.workers.len() {
            self.spawn_one(&self.workers[i]);
        }
    }
}

impl<W: Worker> Default for WorkerGroup<W> {
    fn default() -> Self {
        Self::new(num_cpus::get() / 4)
    }
}
