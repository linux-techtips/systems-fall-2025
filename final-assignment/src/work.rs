use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::thread;
use std::time::Instant;

use crate::ring::AtomicRingBuffer;

/// Spawns, synchronizes, and schedules Tasks for Workers to execute.
pub struct Queue {
    work: AtomicRingBuffer<Task, 1024>,
    waker: Waker,
    sync: Sync,

    total_tasks: AtomicUsize,
    total_error: AtomicUsize,
}

impl Queue {
    /// Creates a new Queue.
    pub fn new() -> Self {
        Self {
            work: AtomicRingBuffer::new(),
            waker: Waker::new(),
            sync: Sync::new(),

            total_tasks: 0.into(),
            total_error: 0.into(),
        }
    }

    /// Schedules a task to be executed and will spawn a new worker or wake an existing worker to
    /// execute it.
    pub fn spawn(&self, task: impl Into<Task>) {
        let success = self.work.push(task.into());
        debug_assert!(success, "work queue full");

        self.total_tasks.fetch_add(1, Ordering::Relaxed);

        if self.sync.get_spawned() < 12 {
            Worker::spawn(self);
        } else {
            self.waker.wake_one();
        }
    }

    /// Waits for all currently scheduled tasks to be executed to completion.
    pub fn wait(&mut self) {
        while !self.sync.all_idle() {
            // TODO: A barrier or something rather than a thread spin loop.
            thread::yield_now();
        }

        self.sync.toggle_join();

        loop {
            self.waker.wake_all();

            if self.sync.get_spawned() == 0 {
                break;
            }

            // TODO: A barrier or something rather than a thread spin loop.
            thread::yield_now();
        }

        self.sync.toggle_join();

        debug_assert!(self.work.is_empty());
    }

    /// Returns metrics about the Queue.
    pub fn metrics(&self) -> Metrics {
        Metrics {
            current_spawned: self.sync.get_spawned(),
            current_idle: self.sync.get_idle(),
            current_work: self.work.len(),
            total_tasks: self.total_tasks.load(Ordering::Relaxed),
            total_error: self.total_error.load(Ordering::Relaxed),
            is_joining: self.sync.get_joining(),
            timestamp: Instant::now(),
        }
    }
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        self.wait();
    }
}

/// A wrapper that exposes the Worker context to tasks being executed by the worker.
pub struct Scope(*mut Worker);

impl Scope {
    /// Schedules a Task to run in the Scope's Worker.
    pub fn schedule(&self, priority: Priority, task: impl Into<Task>) {
        // SAFETY: See `Self::worker()`
        let worker = unsafe { self.worker() };
        let task = task.into();

        match priority {
            Priority::Realtime => worker.work.push_front(task),
            Priority::Default => worker.work.push_back(task),
            Priority::Background => {
                worker.queue.work.push(task);
            }
        };

        worker.queue.total_tasks.fetch_add(1, Ordering::Relaxed);
    }

    /// Returns true if the Worker is shutting down. Otherwise returns false.
    pub fn is_shutdown(&self) -> bool {
        // SAFETY: See `Self::worker()`
        let worker = unsafe { self.worker() };

        worker.queue.sync.get_joining()
    }

    // SAFETY: The scope will only be used within the context of the current running task. The
    // lifetime of the Worker exceeds the lifetime of the Scope. Relative to the Task, the Scope
    // and the *mut Worker can be treated as 'static. The Scope API also ensures we will not
    // violate the guarantees that make this possible.
    unsafe fn worker(&self) -> &'static mut Worker {
        unsafe { core::mem::transmute(&mut *self.0) }
    }
}

/// Implements priority in which tasks can be scheduled within a worker.
pub enum Priority {
    Realtime,
    Default,
    Background,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Default
    }
}

/// Units of execution spawned from a Queue that exeucte Tasks.
struct Worker {
    queue: &'static Queue,
    work: VecDeque<Task>,
}

impl Worker {
    /// Spawnd a worker from the context of a Queue.
    fn spawn(queue: &Queue) {
        if queue.sync.get_joining() {
            return;
        }

        queue.sync.inc_spawned();

        let queue = queue as *const Queue as usize;

        thread::spawn(move || {
            // SAFETY: Normally this would be extremely unsafe. However, the static Queue reference
            // will only be used by the Workers spawned in the Queue. In `Queue::drop`, we wait for
            // all spawned Workers to complete and this queue reference to be dropped. Therefore,
            // according to the perspective of the current Worker, the Queue reference is 'static.
            // This is also a shared reference and not an exclusive reference therefore it is safe
            // to be accessed across threads.
            let queue: &'static Queue = unsafe { core::mem::transmute(&*(queue as *const Queue)) };

            let mut worker = Worker::new(queue);

            'work: loop {
                worker.work();

                if queue.sync.get_joining() {
                    break 'work;
                }

                queue.sync.inc_idle();
                queue.waker.sleep();
                queue.sync.dec_idle();
            }

            queue.sync.dec_spawned();
        });
    }

    /// Executes available tasks.
    fn work(&mut self) {
        // Steals work from the global work queue into the local work queue.
        self.queue.work.steal(&mut self.work);

        // Executes task on the local work in priority order.
        while let Some(task) = self.work.pop_front() {
            self.execute(task);
        }

        // Attempt to execute more tasks from the global work queue if there is any left.
        while let Some(task) = self.queue.work.pop() {
            self.execute(task);
        }
    }

    /// Executes a task within a Worker context.
    fn execute(&mut self, task: Task) {
        let scope = Scope(self as *mut Self);
        let panic = panic::catch_unwind(AssertUnwindSafe(move || task.0(scope)));

        if panic.is_err() {
            self.queue.total_error.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Creates a new Worker.
    fn new(queue: &'static Queue) -> Self {
        Self {
            work: VecDeque::with_capacity(256),
            queue,
        }
    }
}

/// Wrapper type for units of work that workers can execute.
/// This should just be a single pointer and a pointer to the Scope but NOOOOO Rust forces Fn() and
/// friends to be Boxed.
#[repr(transparent)]
pub struct Task(Box<dyn FnOnce(Scope) + Send + 'static>);

impl<F> From<F> for Task
where
    F: FnOnce(Scope) + Send + 'static,
{
    fn from(value: F) -> Self {
        Self(Box::new(value))
    }
}

#[derive(Debug)]
pub struct Metrics {
    pub current_spawned: u16,
    pub current_idle: u16,
    pub current_work: u32,
    pub total_tasks: usize,
    pub total_error: usize,
    pub is_joining: bool,
    pub timestamp: Instant,
}

/// A lightweight integer used for cross-worker synchronization.
#[repr(transparent)]
struct Sync(AtomicU32);

impl Sync {
    const JOIN_MASK: u32 = 1 << 31;

    const SPAWN_MASK: u32 = 0x3FFF << 14;
    const IDLED_MASK: u32 = 0x3FFF;

    /// Creates a new `Sync`
    fn new() -> Self {
        Sync(0.into())
    }

    /// Checks if the JOIN flag is set.
    fn get_joining(&self) -> bool {
        self.0.load(Ordering::Acquire) & Self::JOIN_MASK != 0
    }

    /// Toggles the JOIN flag.
    fn toggle_join(&self) {
        self.0.fetch_xor(Self::JOIN_MASK, Ordering::AcqRel);
    }

    /// Returns the number of workers currently spawned.
    fn get_spawned(&self) -> u16 {
        ((self.0.load(Ordering::Acquire) & Self::SPAWN_MASK) >> 14) as u16
    }

    /// Increments the number of workers currently spawned.
    fn inc_spawned(&self) -> u16 {
        (((self.0.fetch_add(1 << 14, Ordering::AcqRel) + (1 << 14)) & Self::SPAWN_MASK) >> 14)
            as u16
    }

    /// Decrements the number of workers currently spawned.
    fn dec_spawned(&self) -> u16 {
        (((self.0.fetch_sub(1 << 14, Ordering::AcqRel) - (1 << 14)) & Self::SPAWN_MASK) >> 14)
            as u16
    }

    /// Returns the number of idle workers.
    fn get_idle(&self) -> u16 {
        (self.0.load(Ordering::Acquire) & Self::IDLED_MASK) as u16
    }

    /// Increments the number of idle workers.
    fn inc_idle(&self) -> u16 {
        ((self.0.fetch_add(1, Ordering::AcqRel) + 1) & Self::IDLED_MASK) as u16
    }

    /// Decrements the number of idle workers.
    fn dec_idle(&self) -> u16 {
        ((self.0.fetch_sub(1, Ordering::AcqRel) - 1) & Self::IDLED_MASK) as u16
    }

    /// Returns true if all spawned threads are idle. Otherwise returns false.
    fn all_idle(&self) -> bool {
        let state = self.0.load(Ordering::Acquire);

        let idled = state & Self::IDLED_MASK;
        let total = (state & Self::SPAWN_MASK) >> 14;

        idled == total
    }
}

/// Synchornizes workers by allowing them to wait for more work.
struct Waker(Condvar, Mutex<u64>);

impl Waker {
    /// Creates a new `Waker`
    fn new() -> Self {
        Self(Condvar::new(), Mutex::new(0))
    }

    /// Wakes a thread waiting on the `Waker`.
    fn wake_one(&self) {
        let mut seq = self.1.lock().unwrap();
        *seq = seq.wrapping_add(1);

        self.0.notify_one();
    }

    /// Wakes all threads waiting on the `Waker`.
    fn wake_all(&self) {
        let mut seq = self.1.lock().unwrap();
        *seq = seq.wrapping_add(1);

        self.0.notify_all();
    }

    /// Waits for another thread to `Self::wake_one` or `Self::wake_all`
    fn sleep(&self) {
        let mut seq = self.1.lock().unwrap();
        let current = *seq;

        while *seq == current {
            seq = self.0.wait(seq).unwrap();
        }
    }
}
