# oxygen_bot

An IRC bot written in Rust that allows you to define factoids and use them
later.

The primary use case is for channels that are for learning a specific topic,
such as programming, which often have repetitive responses for most
problems/questions.

Credit goes to [darkf](https://github.com/darkf) with his
[lambot](https://github.com/darkf/lambot) project for the idea of this kind of bot. 

I wanted to write a version in a language I understood, so that's what I
did.

# Usage

First off, you should edit the `oxygen_config.toml` language to your liking.
There are very few keys, and I'm pretty sure they are self-explanatory.

To proceed further, you will need the Rust programming language. You can get
it in the most simple way at [rustup.rs](https://www.rustup.rs/).

Then, typing `cargo run --release` into your terminal will compile all the
needed libraries (only the first time!) and then run the bot.
