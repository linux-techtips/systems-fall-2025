enum Fruit {
    Apple(String),
    Banana(String),
    Tomato(String),
}

struct Inventory {
    fruit: Vec<Fruit>,
}

impl Inventory {
    fn available_fruits(&self) {
        println!("We have {} types of fruits available.", self.fruit.len());
    }

    fn tell_me_joke(fruit: &Fruit) {
        match fruit {
            Fruit::Apple(joke) | Fruit::Banana(joke) | Fruit::Tomato(joke) => {
                println!("{}", joke);
            }
        }
    }
}

fn main() {
    let a = "What do you call an apple that plays the trumpet? A tooty fruity!".to_string();
    let b = "Why don't bananas ever feel lonely? Because they hang around in bunches!".to_string();
    let t = "Why did the tomato turn red? Because it saw the salad dressing!".to_string();
    let fruits = vec![Fruit::Banana(b), Fruit::Apple(a), Fruit::Tomato(t)];
    let grocery_store = Inventory { fruit: fruits };

    Inventory::tell_me_joke(&grocery_store.fruit[0]);

    grocery_store.available_fruits();
}
