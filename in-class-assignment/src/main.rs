struct Student {
    name: String,
    major: String,
}

impl Student {
    fn new(name: String, major: String) -> Self {
        Self { name, major }
    }

    fn set_major(&mut self, major: String) {
        self.major = major;
    }

    fn get_major(&self) -> &str {
        &self.major
    }
}

fn main() {
    let mut student = Student::new("Carter".into(), "Computer Science".into());

    println!("{}'s major is {}", student.name, student.get_major());

    student.set_major("Mathematics".into());

    println!("{}'s major is now {}", student.name, student.get_major());
}
