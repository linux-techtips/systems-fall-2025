#![cfg_attr(debug_assertions, allow(unused))]

mod ring;
mod util;
mod work;

use work::{Priority, Queue, Scope};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

struct Monitor {
    queue: Arc<Queue>,
    completed_tasks: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
}

impl Monitor {
    fn new(queue: Arc<Queue>) -> Self {
        Self {
            queue,
            completed_tasks: Arc::new(AtomicUsize::new(0)),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    fn spawn_monitor_thread(&self) -> thread::JoinHandle<()> {
        let queue = self.queue.clone();
        let shutdown = self.shutdown.clone();

        thread::spawn(move || {
            let mut last_metrics = queue.metrics();
            println!("=== Starting Queue Monitor ===\n");

            while !shutdown.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(500));

                let metrics = queue.metrics();
                let elapsed = metrics.timestamp.duration_since(last_metrics.timestamp);

                let tasks_processed = metrics.total_tasks.saturating_sub(last_metrics.total_tasks);
                let errors_occurred = metrics.total_error.saturating_sub(last_metrics.total_error);

                let throughput = if elapsed.as_secs_f64() > 0.0 {
                    tasks_processed as f64 / elapsed.as_secs_f64()
                } else {
                    0.0
                };

                let active_workers = metrics.current_spawned - metrics.current_idle;
                let error_rate = if metrics.total_tasks > 0 {
                    (metrics.total_error as f64 / metrics.total_tasks as f64) * 100.0
                } else {
                    0.0
                };

                println!("┌─────────────────────────────────────────────┐");
                println!("│ Queue Metrics                               │");
                println!("├─────────────────────────────────────────────┤");
                println!(
                    "│ Queue Size:       {:6} tasks              │",
                    metrics.current_work
                );
                println!(
                    "│ Workers:          {:2} total, {:2} active, {:2} idle │",
                    metrics.current_spawned, active_workers, metrics.current_idle
                );
                println!("├─────────────────────────────────────────────┤");
                println!(
                    "│ Total Tasks:      {:8}                   │",
                    metrics.total_tasks
                );
                println!(
                    "│ Total Errors:     {:8}                   │",
                    metrics.total_error
                );
                println!("│ Error Rate:       {error_rate:6.2}%                  │");
                println!("├─────────────────────────────────────────────┤");
                println!("│ Throughput:       {throughput:8.1} tasks/sec         │");
                println!(
                    "│ Joining:          {:5}                     │",
                    metrics.is_joining
                );
                println!("└─────────────────────────────────────────────┘\n");

                if errors_occurred > 0 {
                    println!("⚠️  WARNING: {errors_occurred} errors in last interval");
                }

                if metrics.current_work > 200 {
                    println!(
                        "⚠️  WARNING: Queue depth high ({} tasks)",
                        metrics.current_work
                    );
                }

                if active_workers == 0 && metrics.current_work > 0 {
                    println!(
                        "❌ CRITICAL: No active workers but queue has {} tasks!",
                        metrics.current_work
                    );
                }

                last_metrics = metrics;
            }

            println!("=== Monitor Shutdown ===");
        })
    }

    fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    fn wrap_task<F>(&self, f: F) -> impl FnOnce(Scope) + Send + 'static
    where
        F: FnOnce(Scope) + Send + 'static,
    {
        let completed = self.completed_tasks.clone();
        move |scope| {
            f(scope);
            completed.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn completed_count(&self) -> usize {
        self.completed_tasks.load(Ordering::Relaxed)
    }
}

fn main() {
    let queue = Arc::new(Queue::new());
    let monitor = Monitor::new(queue.clone());

    let monitor_handle = monitor.spawn_monitor_thread();

    println!("Submitting 100 normal tasks...\n");
    for i in 0..100 {
        let task = monitor.wrap_task(move |scope: Scope| {
            thread::sleep(Duration::from_millis(10 + (i % 20) * 5));

            if i.is_multiple_of(10) {
                scope.schedule(Priority::Realtime, |_| {
                    thread::sleep(Duration::from_millis(5));
                });
            }
        });
        queue.spawn(task);
    }

    thread::sleep(Duration::from_secs(2));

    println!("\nSubmitting 20 tasks with some failures...\n");
    for i in 0..20 {
        let task = monitor.wrap_task(move |_scope: Scope| {
            thread::sleep(Duration::from_millis(50));
            if i % 5 == 0 {
                panic!("Simulated task failure {i}");
            }
        });
        queue.spawn(task);
    }

    thread::sleep(Duration::from_secs(3));

    println!("\nStress test: 500 quick tasks...\n");
    for i in 0..500 {
        let task = monitor.wrap_task(move |scope: Scope| {
            thread::sleep(Duration::from_millis(1));

            let priority = match i % 3 {
                0 => Priority::Background,
                1 => Priority::Default,
                _ => Priority::Realtime,
            };

            scope.schedule(priority, |_| {
                thread::sleep(Duration::from_micros(100));
            });
        });
        queue.spawn(task);
    }

    thread::sleep(Duration::from_secs(5));

    monitor.stop();
    monitor_handle.join().unwrap();

    println!("\n=== Final Statistics ===");
    let final_metrics = queue.metrics();
    println!("Total tasks submitted:  {}", final_metrics.total_tasks);
    println!("Total errors:           {}", final_metrics.total_error);
    println!("Tasks completed:        {}", monitor.completed_count());
    println!(
        "Success rate:           {:.2}%",
        (monitor.completed_count() as f64 / final_metrics.total_tasks as f64) * 100.0
    );
}
