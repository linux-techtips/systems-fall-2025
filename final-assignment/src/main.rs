mod ring;
mod util;
mod work;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

pub use work::{Priority, Queue, Scope};

/// This is a benchmark that covers latency, worker concurrency, and task throughput.
/// We have up to 100 workers all operating on 10k tasks.
/// On my M4 Pro running this test in a pretty noisy environment task latency is about 5us and it
/// takes about 6ms to complete all 10k tests. The numbers do get slightly better when you reduce
/// the worker count to the available parallelism.
fn main() {
    println!("run cargo test for behavioral tests");

    let tracker = LatencyTracker::new();
    let mut queue = Queue::builder().max_workers(100).build();

    for _ in 0..1024 {
        queue.spawn(tracker.track(move |scope: Scope| {
            for _ in 0..10 {
                scope.schedule(Priority::Default, |_scope: Scope| {
                    core::hint::black_box(34 + 35);
                });
            }
        }));
    }

    queue.wait();

    println!("Mean Latency: {:.2}us", tracker.mean_us());
    println!("Median Bucket: {}", tracker.median_bucket());
}

struct LatencyTracker {
    buckets: [AtomicUsize; 6],
    total_latency_us: AtomicU64,
    total_samples: AtomicUsize,
}

impl LatencyTracker {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            buckets: Default::default(),
            total_latency_us: 0.into(),
            total_samples: 0.into(),
        })
    }

    fn track<F>(self: &Arc<Self>, f: F) -> impl FnOnce(Scope) + Send + 'static
    where
        F: FnOnce(Scope) + Send + 'static,
    {
        let tracker = Arc::clone(self);
        let started = Instant::now();

        move |scope: Scope| {
            let latency = started.elapsed();
            tracker.record(latency);

            f(scope);
        }
    }

    fn record(&self, latency: Duration) {
        let us = latency.as_micros() as u64;

        self.total_latency_us.fetch_add(us, Ordering::Relaxed);
        self.total_samples.fetch_add(1, Ordering::Relaxed);

        let idx = match us {
            0..=1 => 0,
            2..=10 => 1,
            11..=100 => 2,
            101..=1000 => 3,
            1001..=10000 => 4,
            _ => 5,
        };

        self.buckets[idx].fetch_add(1, Ordering::Relaxed);
    }

    fn mean_us(&self) -> f64 {
        let total = self.total_latency_us.load(Ordering::Relaxed);
        let count = self.total_samples.load(Ordering::Relaxed);

        total as f64 / count as f64
    }

    fn median_bucket(&self) -> usize {
        let total = self.total_samples.load(Ordering::Relaxed);
        let mut cum = 0;

        for (i, bucket) in self.buckets.iter().enumerate() {
            cum += bucket.load(Ordering::Relaxed);
            if cum >= total / 2 {
                return i;
            }
        }

        5
    }
}
