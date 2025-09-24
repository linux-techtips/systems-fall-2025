const FREEZING_POINT_FARENHEIGHT: f64 = 32.0;

fn farenheight_to_celsius(farenheight: f64) -> f64 {
    (farenheight - FREEZING_POINT_FARENHEIGHT) * 5.0 / 9.0
}

fn celsius_to_farenheight(celsius: f64) -> f64 {
    (celsius * 9.0 / 5.0) + FREEZING_POINT_FARENHEIGHT
}

fn assignment1() {
    {
        let mut farenheight = 100.0;

        for i in 0..=5 {
            farenheight += i as f64;

            println!(
                "{farenheight}°F is {:.2}°C",
                farenheight_to_celsius(farenheight)
            );
        }
    }
    {
        let mut celsius = 24.0;

        for i in 0..=5 {
            celsius += i as f64;

            println!("{celsius}°C is {:.2}°F", celsius_to_farenheight(celsius));
        }
    }
}

fn is_even(n: i32) -> bool {
    n & 1 == 0
}

fn assignment2() {
    let nums = [1, 2, 3, 4, 5];

    for num in nums {
        println!(
            "{} is even: {}",
            num,
            if is_even(num) { "even" } else { "odd" }
        );

        match (num % 3 == 0, num % 5 == 0) {
            (true, false) => println!("Fizz"),
            (false, true) => println!("Buzz"),
            (true, true) => println!("FizzBuzz"),
            _ => println!("FizzBuzz"),
        }
    }

    {
        let mut sum = 0;
        let mut i = 0;

        while i < nums.len() {
            sum += nums[i];
            i += 1;
        }

        println!("Sum: {}", sum);
    }

    {
        let mut max = nums[0];
        let mut i = 0;
        while i < nums.len() {
            max = max.max(nums[i]);
            i += 1;
        }

        println!("Max: {}", max);
    }
}

fn check_guess(guess: i32, secret: i32) -> i32 {
    if guess < secret {
        -1
    } else if guess > secret {
        1
    } else {
        0
    }
}

fn assignment3() {
    let secret = 42;
    let mut input = String::new();
    let mut count = 1;

    loop {
        std::io::stdin()
            .read_line(&mut input)
            .expect("failed to read line");
        let guess = input.trim().parse::<i32>().expect("input not a number");

        let result = check_guess(guess, secret);
        if result == -1 {
            println!("Too low!");
        } else if result == 1 {
            println!("Too high!");
        } else {
            break;
        }

        count += 1;
        input.clear();
    }

    println!("You guessed it in {count} tries!");
}

fn main() {
    assignment1();
    assignment2();
    assignment3();
}
