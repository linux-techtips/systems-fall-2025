#![cfg_attr(debug_assertions, allow(unused))]

trait ShowInfo {
    fn show_info(&self);
}

#[derive(Debug)]
struct Undergrad {
    gpa: f32,
    major: &'static str,
}

impl ShowInfo for Undergrad {
    fn show_info(&self) {
        println!("{self:?}");
    }
}

#[derive(Debug)]
struct Graduate {
    thesis: bool,
    major: &'static str,
    gpa: f32,
}

impl ShowInfo for Graduate {
    fn show_info(&self) {
        println!("{self:?}");
    }
}

struct Enrollment<'a> {
    students: Vec<&'a dyn ShowInfo>,
}

impl<'a> ShowInfo for Enrollment<'a> {
    fn show_info(&self) {
        println!("Enrollment: ");
        self.students.iter().for_each(|s| ShowInfo::show_info(*s))
    }
}

fn main() {
    let undergrads = [
        Undergrad {
            gpa: 3.8,
            major: "CSCI",
        },
        Undergrad {
            gpa: 3.4,
            major: "CYBI",
        },
        Undergrad {
            gpa: 2.0,
            major: "HIST",
        },
    ];

    let graduates = [
        Graduate {
            gpa: 3.0,
            thesis: true,
            major: "CSCI",
        },
        Graduate {
            gpa: 2.0,
            thesis: false,
            major: "CSCI",
        },
    ];

    let enrollment = Enrollment {
        students: Vec::from_iter(
            undergrads
                .iter()
                .map(|u| u as &dyn ShowInfo)
                .chain(graduates.iter().map(|g| g as &dyn ShowInfo)),
        ),
    };

    enrollment.show_info();
}
