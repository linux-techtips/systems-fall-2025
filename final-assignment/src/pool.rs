use crate::ring::AtomicRingBuffer;

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use std::panic::{self, AssertUnwindSafe};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::thread;

pub struct Pool {
    default: AtomicRingBuffer<Task, 256>,
    realtime: AtomicRingBuffer<Task, 256>,
    background: AtomicRingBuffer<Task, 256>,

    sync: Sync,
    waker: Waker,

    total_tasks: AtomicU64,
    total_error: AtomicU64,
}

impl Pool {
    pub fn new() -> Self {
        Self {
            default: AtomicRingBuffer::new(),
            realtime: AtomicRingBuffer::new(),
            background: AtomicRingBuffer::new(),
            sync: Sync::new(),
            waker: Waker::new(),

            total_error: 0.into(),
            total_tasks: 0.into(),
        }
    }

    pub fn schedule(&self, priority: Priority, task: impl Into<Task>) {
        self.total_tasks.fetch_add(1, Ordering::AcqRel);

        match priority {
            Priority::Realtime => self.realtime.push(task.into()),
            Priority::Default => self.default.push(task.into()),
            Priority::Background => self.background.push(task.into()),
        };

        if self.sync.get_idle() != 0 {
            return self.waker.wake_one();
        }

        if self.sync.get_spawned() >= 256 {
            Thread::run(self);
        }
    }

    pub fn metrics(&self) -> Metrics {
        Metrics {
            current_realtime: self.realtime.size(),
            current_default: self.default.size(),
            current_background: self.background.size(),
            total_error: self.total_error.load(Ordering::Acquire),
            total_tasks: self.total_tasks.load(Ordering::Acquire),
        }
    }

    fn work_remaining(&self) -> bool {
        !self.default.is_empty() || !self.realtime.is_empty() || !self.background.is_empty()
    }
}

impl Default for Pool {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.sync.set_shutdown();
        self.waker.wake_all();
        self.sync.wait_all_idle();

        // SAFETY: Since we are in the destructor for the pool we have exclusive access to the
        // pool. I am pinky promising that the reference to the Pool will not leak out past this
        // destructor. See `Scope` and `Thread::execute` for the reasoning.
        let this: &'static Pool = unsafe { core::mem::transmute(self) };

        // TODO: I don't like this
        {
            while let Some(task) = this.realtime.pop() {
                Thread::execute(this, task);
            }

            while let Some(task) = this.default.pop() {
                Thread::execute(this, task);
            }

            while let Some(task) = this.background.pop() {
                Thread::execute(this, task);
            }

            assert!(this.realtime.is_empty());
            assert!(this.default.is_empty());
            assert!(this.background.is_empty());
        }
    }
}

#[repr(u8)]
pub enum Priority {
    Realtime = 0,
    Default = 1,
    Background = 2,
}

#[derive(Debug)]
pub struct Metrics {
    current_realtime: u32,
    current_default: u32,
    current_background: u32,
    total_tasks: u64,
    total_error: u64,
}

/// A unit of work that a worker thread can execute.
#[repr(transparent)]
pub struct Task(Box<dyn FnOnce(Scope) + Send + 'static>);

impl<F> From<F> for Task
where
    F: FnOnce(Scope) + Send + 'static,
{
    fn from(value: F) -> Self {
        Task(Box::new(value))
    }
}

/// A worker thread that runs in the context of a Pool
struct Thread {
    work: Vec<Task>,
}

impl Thread {
    /// Creates and runs a new worker thread to run in the context of the provided Pool.
    fn run(pool: &Pool) {
        let pool = pool as *const Pool as usize;

        thread::spawn(move || {
            // SAFETY: We are unsafely obtaining a &'static Pool from a usize casted pointer from
            // Pool. Why static? According to the lifetime of the thread pool will live forever.
            // `Pool::drop` will wait for all threads to shutdown and finish their work before
            // finishing. From the thread's point of view the pool is 'static because it lives for
            // the entire lifetime of the thread.
            let pool = unsafe { &*(pool as *const Pool) };
            let mut thread = Thread {
                work: Vec::with_capacity(256),
            };

            pool.sync.inc_spawned();

            loop {
                thread.work(pool);

                pool.sync.inc_idle();
                if pool.sync.get_shutdown() {
                    while pool.work_remaining() {
                        thread.work(pool);
                    }

                    break;
                }

                pool.waker.wait_on();
                pool.sync.dec_idle();
            }
        });
    }

    fn work(&mut self, pool: &'static Pool) {
        while let Some(task) = pool.realtime.pop() {
            Self::execute(pool, task);
        }

        if pool.default.steal(&mut self.work) {
            while !self.work.is_empty() {
                for task in self.work.drain(..) {
                    Self::execute(pool, task);
                }
            }
        }

        if let Some(task) = pool.background.pop() {
            Self::execute(pool, task);
        }
    }

    /// Safely executes a `Task` in the context of the provided Pool.
    fn execute(pool: &'static Pool, task: Task) {
        if panic::catch_unwind(AssertUnwindSafe(|| task.0(Scope(pool)))).is_err() {
            pool.total_error.fetch_add(1, Ordering::AcqRel);
        }
    }
}

/// Pool synchronization data that can be safely accessed or modified across threads.
#[repr(transparent)]
struct Sync(AtomicU32);

impl Sync {
    const SHUTDOWN_MASK: u32 = 1 << 31;
    const NOTIFIED_MASK: u32 = 1 << 30;

    const ACTIVE_MASK: u32 = 0x3FFF << 14;
    const EEPERS_MASK: u32 = 0x3FFF;

    fn new() -> Sync {
        Sync(0.into())
    }

    fn get_shutdown(&self) -> bool {
        self.0.load(Ordering::Acquire) & Self::SHUTDOWN_MASK != 0
    }

    fn set_shutdown(&self) {
        self.0.fetch_or(Self::SHUTDOWN_MASK, Ordering::Release);
    }

    fn get_spawned(&self) -> u32 {
        (self.0.load(Ordering::Acquire) & Self::ACTIVE_MASK) >> 14
    }

    fn inc_spawned(&self) {
        self.0.fetch_add(1 << 14, Ordering::AcqRel);
    }

    fn dec_spawned(&self) {
        self.0.fetch_sub(1 << 14, Ordering::AcqRel);
    }

    fn get_idle(&self) -> u32 {
        self.0.load(Ordering::Acquire) & Self::EEPERS_MASK
    }

    fn inc_idle(&self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }

    fn dec_idle(&self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }

    fn all_idle(&self) -> bool {
        let state = self.0.load(Ordering::Acquire);

        let idled = state & Self::EEPERS_MASK;
        let total = (state & Self::ACTIVE_MASK) >> 14;

        idled == total
    }

    fn wait_all_idle(&self) {
        while !self.all_idle() {
            thread::yield_now();
        }
    }

    fn wait_faster(&self) {
        while !self.all_idle() {
            core::hint::spin_loop();
        }
    }

    fn wait_let_it_ride(&self) {
        while !self.all_idle() {}
    }
}

struct Waker(Mutex<u64>, Condvar);

impl Waker {
    fn new() -> Self {
        Self(Mutex::new(0), Condvar::new())
    }

    fn wake_one(&self) {
        let (mut seq, cvar) = self.guard();
        *seq = seq.wrapping_add(1);

        cvar.notify_one();
    }

    fn wake_all(&self) {
        let (mut seq, cvar) = self.guard();
        *seq = seq.wrapping_add(1);

        cvar.notify_all();
    }

    fn wait_on(&self) {
        let (mut seq, cvar) = self.guard();
        let current = *seq;

        while *seq == current {
            seq = cvar.wait(seq).unwrap();
        }
    }

    fn guard(&self) -> (MutexGuard<'_, u64>, &Condvar) {
        (self.0.lock().unwrap(), &self.1)
    }
}

/// A safe wrapper around a Pool that exposes operations that can be called from any spawned
/// thread.
pub struct Scope(&'static Pool);

impl Scope {
    /// Schedule a task into the `Pool`
    pub fn schedule(&self, priority: Priority, task: impl Into<Task>) {
        self.0.schedule(priority, task);
    }

    /// Check if the `Pool` is currently shutting down.
    pub fn is_shutdown(&self) -> bool {
        self.0.sync.get_shutdown()
    }
}
