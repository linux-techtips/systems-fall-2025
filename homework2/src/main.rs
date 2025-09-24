fn concat_strings(s1: &String, s2: &String) -> String {
    [s1.as_str(), s2.as_str()].concat()
}

fn clone_and_modify(s: &String) -> String {
    let mut s = s.clone();

    s.push_str(", modified");

    s
}

fn sum(total: &mut i32, low: i32, high: i32) {
    *total = (low..=high).sum();
}

fn main() {
    // Problem #1
    {
        let s1 = "Hello".to_string();
        let s2 = ", World".to_string();

        let result = concat_strings(&s1, &s2);

        println!("[PROBLEM 1] {s1:?} + {s2:?} = {result:?}");

        assert_eq!(result.as_str(), "Hello, World");
    }

    // Problem #2
    {
        let s = "Hello".to_string();
        let modified = clone_and_modify(&s);

        println!("[PROBLEM 2] Original: {s:?}, Modified: {modified:?}");

        assert_eq!(modified.as_str(), "Hello, modified");
    }

    // Problem #3
    {
        let mut total = 0;
        sum(&mut total, 0, 100);

        println!("[PROBLEM 3] Sum from 0 to 100 is {total}");

        assert_eq!(total, 5050);
    }
}
