#[macro_use] extern crate lazy_static;
extern crate regex;

use regex::Regex;

use std::io::{Read, Write};
use std::net::TcpStream;

#[derive(Debug)]
enum Sender {
    Server(String),
    User(String, String, String),
    Nobody,
}

#[derive(Debug)]
struct IrcMessage {
    sender: Sender,
    command: String,
    params: Vec<String>,
}


impl IrcMessage {
    fn new(line: &str) -> IrcMessage {
        lazy_static! {
            static ref LINE_REGEXP: Regex = Regex::new(r"(:([^! ]+)(?:!([^@]+)@(\S+))? )?(\w+)(?: (.+))?").unwrap();
        }

        let captures = match LINE_REGEXP.captures(line) {
            Some(x) => x,
            None => {
                println!("{:?}", line);
                panic!("Wrongful line.")
            }
        };
        let get_capture = |n| String::from(captures.get(n).unwrap().as_str());

        let sender;
        if let None = captures.get(1) {
            sender = Sender::Nobody;
        } else {
            sender = match captures.get(3) {
                // We assume that since there is a username and hostname this is an User.
                Some(_) => Sender::User(get_capture(2), get_capture(3), get_capture(4)),
                None => Sender::Server(get_capture(2))
            }
        }

        IrcMessage {
            sender: sender,
            command: get_capture(5),
            params: IrcMessage::split_params(get_capture(6)),
        }
    }

    fn split_params(params: String) -> Vec<String> {
        let mut output: Vec<String> = Vec::new();
        let mut parts = params.split_whitespace();

        while let Some(part) = parts.next() {
            if part.starts_with(":") {
                // We skip the colon.
                let mut joined = String::from(&part[1..]);

                for part in parts {
                    joined.push(' ');
                    joined.push_str(part);
                }

                output.push(joined);
                break;
            } else {
                output.push(String::from(part));
            }
        }

        output
    }
}

#[derive(Debug)]
struct OxygenBot {
    stream: TcpStream
}

impl OxygenBot {
    fn new(host: &str, port: u16) -> Self {
        OxygenBot {
            stream: TcpStream::connect((host, port)).unwrap(),
        }
    }

    fn read_lines(&mut self) -> Vec<String> {
        let mut read_buf = [0u8; 1024];
        let mut lines_buf = String::new();

        while !lines_buf.ends_with("\r\n") {
            let n = self.stream.read(&mut read_buf).unwrap();
            if n == 0 { break; }
            lines_buf += &String::from_utf8_lossy(&read_buf[..n]).into_owned()[..];
        }

        return lines_buf.split("\r\n").map(String::from).collect();
    }

    fn send_line(&mut self, line: &str) {
        write!(self.stream, "{}\r\n", line).unwrap();
    }

    fn mainloop(&mut self) {
        self.send_line("NICK OxygenBot");
        self.send_line("USER OxygenBot * * :OxygenBot");

        loop {
            for line in self.read_lines() {
                if line.len() == 0 { break; }
                let message = IrcMessage::new(&line[..]);

                match &message.command[..] {
                    "PING" => self.send_line(&line.replace("PING", "PONG")),
                    _ => continue,
                }
            }
        }
    }
}

fn main() {
    let mut bot = OxygenBot::new("chat.freenode.net", 6667);
    bot.mainloop();
}
