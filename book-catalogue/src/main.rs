#![cfg_attr(debug_assertions, allow(dead_code, unused_imports))]

use std::fs::File;
use std::io::{BufRead, BufReader, Write};

struct Book {
    title: String,
    author: String,
    year: u16,
}

fn save_books(books: &Vec<Book>, filenme: &str) {
    let mut file = File::create(filenme).expect("Unable to create file");

    for book in books {
        writeln!(file, "{},{},{}", book.title, book.author, book.year)
            .expect("Unable to write data");
    }
}

fn load_books(filename: &str) -> Vec<Book> {
    let file = File::open(filename).expect("Unable to open file");
    let reader = BufReader::new(file);

    let mut books = Vec::new();
    let mut it = reader.lines();

    while let Some(Ok(line)) = it.next() {
        let [title, author, year] = line.split(',').collect::<Vec<_>>().try_into().unwrap();

        books.push(Book {
            title: title.to_string(),
            author: author.to_string(),
            year: year.parse().unwrap(),
        })
    }

    books
}

fn main() {
    let books = vec![
        Book {
            title: "1984".to_string(),
            author: "George Orwell".to_string(),
            year: 1949,
        },
        Book {
            title: "To Kill a Mockingbird".to_string(),
            author: "Harper Lee".to_string(),
            year: 1960,
        },
    ];

    save_books(&books, "books.txt");
    println!("Books saved to file.");

    let loaded_books = load_books("books.txt");
    println!("Loaded books:");

    for book in loaded_books {
        println!(
            "{} by {}, published in {}",
            book.title, book.author, book.year
        );
    }
}
