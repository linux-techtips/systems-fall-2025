#![cfg_attr(debug_assertions, allow(unused))]
#![feature(extend_one)]

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicUsize, Ordering};

use std::sync::{Arc, Barrier};
use std::thread;

mod pool;
mod ring;
mod util;

type Stack = [MaybeUninit<u8>; 256];

static mut SHARED_STACK: Stack = [const { MaybeUninit::uninit() }; 256];

struct Thread {
    stack: Stack,
    index: usize,
    batch: &'static Batch,
}

impl Thread {
    thread_local! {
        static CURRENT: UnsafeCell<*mut Thread> = const { UnsafeCell::new(core::ptr::null_mut()) };
    }

    pub fn total() -> usize {
        Self::current().batch.total
    }

    pub fn index() -> usize {
        Self::current().index
    }

    pub fn wait() {
        Self::current().batch.barrier.wait();
    }

    pub fn range(count: usize) -> core::ops::Range<usize> {
        let thread = Self::current();

        let (work, left) = (count / thread.batch.total, count % thread.batch.total);
        let has_leftover = thread.index < left;

        let leftover = if has_leftover { thread.index } else { left };
        let start = work * thread.index + leftover;

        start..(start + work + has_leftover as usize)
    }

    pub fn sync<T: Send>(index: usize, data: &mut T) {
        const { assert!(core::mem::size_of::<T>() <= core::mem::size_of::<Stack>()) };

        let thread = Self::current();

        if thread.index == index {
            unsafe {
                #[allow(static_mut_refs)]
                core::ptr::copy_nonoverlapping(
                    data as *const T as *const u8,
                    SHARED_STACK.as_mut_ptr() as *mut u8,
                    core::mem::size_of::<T>(),
                );
            }
        }
        thread.batch.barrier.wait();

        if thread.index != index {
            unsafe {
                #[allow(static_mut_refs)]
                core::ptr::copy_nonoverlapping(
                    SHARED_STACK.as_ptr() as *const u8,
                    data as *mut T as *mut u8,
                    core::mem::size_of::<T>(),
                );
            }
        }
        thread.batch.barrier.wait();
    }

    fn current() -> &'static mut Self {
        Self::CURRENT.with(|local| unsafe { &mut **local.get() })
    }

    fn work(batch: &'static Batch, index: usize, task: impl FnOnce() + Send + 'static) {
        let mut thread = Thread::new(batch, index);
        Thread::CURRENT.with(|local| unsafe { local.get().write(&mut thread) });

        task();

        batch.counter.fetch_add(1, Ordering::AcqRel);
    }

    fn new(batch: &'static Batch, index: usize) -> Self {
        Self {
            stack: [const { MaybeUninit::uninit() }; 256],
            index,
            batch,
        }
    }
}

struct Batch {
    counter: AtomicUsize,
    barrier: Barrier,
    total: usize,
}

impl Batch {
    fn execute(total: usize, task: impl FnOnce() + Send + Clone + 'static) {
        assert!(total > 0);

        let counter = AtomicUsize::new(0);
        let barrier = Barrier::new(total);

        let batch = Self {
            counter: AtomicUsize::new(0),
            barrier: Barrier::new(total),
            total,
        };

        let batch: &'static Batch = unsafe { core::mem::transmute(&batch) };

        for index in 1..total {
            let task = task.clone();
            thread::spawn(move || Thread::work(batch, index, task));
        }

        Thread::work(batch, 0, task);

        while batch.counter.load(Ordering::Acquire) < total {
            core::hint::spin_loop();
        }
    }
}

fn main() {
    let mut data = unsafe { Box::<[usize]>::new_uninit_slice(1024).assume_init() };

    let send = (data.as_mut_ptr() as usize, data.len());

    Batch::execute(10, move || {
        let mut value = 0;
        if Thread::index() == 0 {
            value = 69;
        }

        Thread::sync(0, &mut value);

        let data: &mut [usize] =
            unsafe { core::slice::from_raw_parts_mut(send.0 as *mut usize, send.1) };

        for i in Thread::range(data.len()) {
            data[i] = value;
        }
    });

    eprintln!("{data:?}");
}
