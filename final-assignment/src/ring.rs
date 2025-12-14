use crate::util::UncheckedAtomics;

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU32, Ordering};

use std::collections::VecDeque;

/// This is the meat and potatoes of the work Queue. The atomic ring buffer allows us to safely
/// enqueue, dequeue, and steal work safely across threads.
/// https://zig.news/kprotty/resource-efficient-thread-pools-with-zig-3291
/// This was based on some of that King Protty has done a while back for the Zig Programming
/// Language.
pub struct AtomicRingBuffer<T, const N: usize> {
    buff: [UnsafeCell<MaybeUninit<T>>; N],
    head: AtomicU32,
    tail: AtomicU32,
}

impl<T, const N: usize> AtomicRingBuffer<T, N> {
    /// Creates a new AtomicRingBuffer.
    pub fn new() -> Self {
        Self {
            // SAFETY:
            //  See `Self::push` and `Self::pop`: Elements will not be read before they are
            //  written.
            //
            //  See `Self::drop`: We assert that the buffer must be drained and all elements
            //  dropped.
            buff: unsafe { MaybeUninit::uninit().assume_init() },
            head: 0.into(),
            tail: 0.into(),
        }
    }

    /// Returns the number of currently allocated elements in the buffer.
    pub fn len(&self) -> u32 {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        tail.wrapping_sub(head)
    }

    /// Returns true if there are no elements in the buffer.
    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);

        head == tail
    }

    /// Pushes an element to the buffer.
    // TODO: push can fail and a viable solution is to spin until it succeeds. However, push
    // requires ownership over the pushed element. Upon failure, this element will not be pushed to
    // the Queue and dropped.
    pub fn push(&self, elem: T) -> bool {
        // SAFETY: We don't need to worry about other threads that have acquired the tail via
        // `Self::pop` and `Self::steal`. We only check to see if the queue if full. Removing
        // elements from the queue would only cause potential failures to succeed.
        let tail = unsafe { self.tail.load_unchecked() };
        let head = self.head.load(Ordering::Acquire);

        if tail.wrapping_sub(head) as usize >= self.buff.len() {
            return false;
        }

        let slot = &self.buff[tail as usize & (self.buff.len() - 1)];
        // SAFETY: We are moving ownership of the element into the buffer without requiring an
        // exclusive reference to self. This operation is safe as we are preventing multiple
        // threads from writing to the slot at the same time.
        unsafe { core::ptr::write((*slot.get()).as_mut_ptr(), elem) };

        self.tail.store(tail.wrapping_add(1), Ordering::Release);

        true
    }

    /// Removes an element from the buffer.
    pub fn pop(&self) -> Option<T> {
        loop {
            // SAFETY: We don't need to worry about other threads that have acquired the tail via
            // `Self::pop` and `Self::steal`. We only check to see if the queue if full. Removing
            // elements from the queue would only cause potential failures to succeed.
            let tail = unsafe { self.tail.load_unchecked() };
            let head = self.head.load(Ordering::Relaxed);

            if head == tail {
                return None;
            }

            let steal = self.head.compare_exchange_weak(
                head,
                head.wrapping_add(1),
                Ordering::AcqRel,
                Ordering::Acquire,
            );

            if steal.is_ok() {
                let slot = &self.buff[head as usize & (self.buff.len() - 1)];
                // SAFETY: We are moving ownership of the element from the buffer to the caller. At
                // this point we have guaranteed that there is an initialized element to pop and we
                // are preventing another thread from having access to the slot.
                let elem = unsafe { core::ptr::read((*slot.get()).as_ptr()) };

                return Some(elem);
            }

            core::hint::spin_loop()
        }
    }

    /// Moves ownership of multiple elements in the buffer to the destination at once.
    pub fn steal(&self, dest: &mut VecDeque<T>) -> bool {
        loop {
            let head = self.head.load(Ordering::Acquire);
            let tail = self.tail.load(Ordering::Acquire);

            if head == tail {
                return false;
            }

            let size = tail.wrapping_sub(head);
            if size as usize > self.buff.len() {
                core::hint::spin_loop();
                continue;
            }

            let steal_size = size - (size / 2);
            if steal_size == 0 {
                return false;
            }

            let steal = self.head.compare_exchange(
                head,
                head.wrapping_add(steal_size),
                Ordering::AcqRel,
                Ordering::Acquire,
            );

            if steal.is_ok() {
                dest.reserve(steal_size as usize);

                for i in 0..steal_size {
                    let slot = &self.buff[head.wrapping_add(i) as usize & (self.buff.len() - 1)];
                    // SAFETY: We are moving ownership of the element from the buffer to the caller. At
                    // this point we have guaranteed that there is an initialized element to pop and we
                    // are preventing another thread from having access to the slot.
                    let elem = unsafe { core::ptr::read((*slot.get()).as_ptr()) };
                    dest.push_back(elem);
                }

                return true;
            }

            core::hint::spin_loop();
        }
    }

    // This is a suprise tool that we use to make sure the size of the Ring Buffer is a power of
    // two. This allows us to replace modulo in ring operations like: head % buff.len() into head &
    // (buff.len() - 1) which is much faster.
    #[doc(hidden)]
    #[allow(dead_code)]
    const ASSERT_SIZE_POW2: () = const { assert!(N & (N - 1) == 0) };
}

impl<T, const N: usize> Drop for AtomicRingBuffer<T, N> {
    fn drop(&mut self) {
        let head = *self.head.get_mut() as usize;
        let tail = *self.tail.get_mut() as usize;

        for i in head..tail {
            let slot = self.buff[i].get() as *mut T;
            // SAFETY: We have exclusive access to the ring buffer. This operation is completely
            // safe unless I've screwed up the head/tail indexing.
            unsafe { core::ptr::drop_in_place(slot) };
        }
    }
}

// SAFETY: AtomicRingBuffer is implemented on top of Atomic Operations and will not leak its
// internal data.
unsafe impl<T: Send, const N: usize> Send for AtomicRingBuffer<T, N> {}
unsafe impl<T: Send, const N: usize> Sync for AtomicRingBuffer<T, N> {}
