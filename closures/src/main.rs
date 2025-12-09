use std::{thread, time::Duration};

fn task1() {
    let operation = |a: i32, b: i32| a * b;
    println!("Result: {}", operation(10, 5));
}

fn track_changes() {
    let mut tracker = 0;
    let mut update = || {
        tracker += 1;
        println!("{tracker}");
    };

    update();
    update();
}

fn task2() {
    track_changes();
}

fn process_vector(vec: Vec<i32>, f: impl Fn(i32) -> i32) -> Vec<i32> {
    let mut new = vec.clone();
    for elem in new.iter_mut() {
        *elem = f(*elem);
    }

    new
}

fn task3() {
    let numbers = vec![1, 2, 3];

    let doubled = process_vector(numbers.clone(), |x| x * 2);
    let replaced = process_vector(numbers, |x| if x > 2 { 0 } else { x });

    println!("Doubled: {doubled:?}");
    println!("Replaced: {replaced:?}");
}

struct ComputeCache<T: Fn() -> String> {
    computation: T,
    cache: Option<String>,
}

impl<T: Fn() -> String> ComputeCache<T> {
    fn new(computation: T) -> Self {
        ComputeCache {
            computation,
            cache: None,
        }
    }

    fn get_result(&mut self) -> String {
        if let Some(cached) = &self.cache {
            return cached.clone();
        }

        self.cache = Some((self.computation)());
        self.cache.clone().unwrap()
    }
}

fn task5() {
    let mut cache = ComputeCache::new(|| {
        println!("Computing (this will take 2 seconds)...");
        thread::sleep(Duration::from_secs(2));
        "Hello, world!".to_string()
    });

    println!("First call:");
    println!("Result: {}", cache.get_result());

    println!("\nSecond call:");
    println!("Result (cached): {}", cache.get_result());
}

fn main() {
    task1();
    task2();
    task3();
    task5();
}

