#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate toml;

use regex::Regex;
use toml::Value;
use std::collections::HashMap;
use std::fs::File;
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
            // regexp groups
            // 1: whole prefix
            // 2: nickname or servername
            // 3: username
            // 4: hostname
            // 5: command
            // 6: params
            static ref LINE_REGEXP: Regex = Regex::new(r"(:([^! ]+)(?:!([^@]+)@(\S+))? )?(\w+)(?: (.+))?").unwrap();
        }

        let captures = LINE_REGEXP.captures(line).expect("Could not match line.");
        let get_capture = |n| String::from(captures.get(n).unwrap().as_str());

        let sender;
        if let None = captures.get(1) {
            // This happens only for PING messages AFAIK.
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
        let mut output = Vec::new();
        let mut parts = params.split_whitespace();

        // TODO: Use a combination of String::find and String::split_at here.
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
struct Factoids {
    filename: String,
    factoids: HashMap<String, String>,
}

impl Factoids {
    fn new(filename: &str) -> Self {
        let mut factoids = HashMap::new();

        if let Ok(mut file) = File::open(filename) {
            let mut contents = String::new();
            file.read_to_string(&mut contents).unwrap();

            for line in contents.split("\n") {
                if let Some(space_index) = line.find(' ') {
                    let (mnemonic, mut response) = line.split_at(space_index);
                    response = response.trim();
                    factoids.insert(String::from(mnemonic), String::from(response));
                }
            }

        }

        Factoids {
            filename: String::from(filename),
            factoids: factoids,
        }
    }

    fn define_factoid(&mut self, name: String, contents: String) {
        self.factoids.insert(name, contents);
        self.save_factoids();
    }

    fn save_factoids(&self) {
        if let Ok(mut file) = File::create(&self.filename) {
            let mut file_contents = String::new();
            for (i, (name, contents)) in self.factoids.iter().enumerate() {
                if i > 0 { file_contents.push('\n'); }
                file_contents.push_str(&format!("{} {}", &name, &contents));
            }
            write!(file, "{}", file_contents).expect("Could not save factoids!");
        }
    }
}

#[derive(Debug)]
struct OxygenBot {
    nickname: String,
    channels: Vec<String>,

    stream: TcpStream,
    factoids: Factoids,
}

impl OxygenBot {
    fn new(nickname: String, channels: Vec<String>, host: &str, port: u16) -> Self {
        OxygenBot {
            nickname: nickname,
            channels: channels,

            stream: TcpStream::connect((host, port)).expect("Could not connect to server!"),
            factoids: Factoids::new("factoids.txt"),
        }
    }

    fn read_lines(&mut self) -> Vec<String> {
        let mut read_buf = [0u8; 1024];
        let mut lines_buf = String::new();

        while !lines_buf.ends_with("\r\n") {
            let n = self.stream.read(&mut read_buf).expect("Could not read from socket.");
            if n == 0 { break; }
            lines_buf += &String::from_utf8_lossy(&read_buf[..n]).into_owned()[..];
        }

        return lines_buf.split("\r\n").map(String::from).collect();
    }

    fn send_line(&mut self, line: &str) {
        write!(self.stream, "{}\r\n", line).expect("Could not send to the server.");
    }

    fn handle_privmsg(&mut self, message: IrcMessage) {
        if !message.params[1].starts_with("$") { return; }

        let text = &message.params[1][1..];

        let factoid: &str;
        let params: Vec<&str>;
        match text.find(' ') {
            Some(n) => {
                let (a, b) = text.split_at(n);
                factoid = a;
                params = b.split_whitespace().collect();
            },
            None => {
                factoid = text;
                params = Vec::new();
            }
        }

        match factoid {
            "defact" if params.len() >= 2 => {
                let factoid_name = String::from(params[0]);
                let mut factoid_contents = String::new();

                for (i, s) in params[1..].iter().enumerate() {
                    if i > 0 { factoid_contents.push(' '); }
                    factoid_contents.push_str(&s);
                }

                if let Sender::User(nick, _, _) = message.sender {
                    self.send_line(&format!("PRIVMSG {} :{}: defined {}",
                                            &message.params[0], &nick, &factoid_name));
                }
                self.factoids.define_factoid(factoid_name, factoid_contents);
            },
            "factoids" => {
                let mut factoid_list = String::new();
                for (i, factoid) in self.factoids.factoids.keys().enumerate() {
                    if i > 0 { factoid_list.push(' '); }
                    factoid_list.push_str(&factoid);
                }

                if let Sender::User(nick, _, _) = message.sender {
                    self.send_line(&format!("PRIVMSG {}, :{}: {}",
                                             &message.params[0], &nick, &factoid_list));
                }
            },
            "at" if params.len() >= 2 => {
                let factoid_name = params[1];
                let recipient = params[0];

                // TODO: Make peace with the borrow checker, here too.
                // We could remove the found variable if the self borrow lifetime didn't extend to
                // the else block.
                // We initialize it to string::new() to remove the warning about potentially
                // uninitialized variable usage.
                let mut value = String::new();
                let mut found = false;

                if let Some(actual_value) = self.factoids.factoids.get(factoid_name) {
                    value = actual_value.clone();
                    found = true;
                }

                if found {
                    self.send_line(&format!("PRIVMSG {} :{}: {}", message.params[0], recipient, value));
                } else if let Sender::User(nick, ..) = message.sender {
                    self.send_line(&format!("PRIVMSG {} :{}: No such factoid: {}",
                                            message.params[0], nick, factoid_name));
                }
            },
            _ => {
                // TODO: Make peace with the borrow checker.
                let value;
                if let Some(actual_value) = self.factoids.factoids.get(factoid) {
                    value = actual_value.clone();
                } else {
                    return;
                }

                self.send_line(&format!("PRIVMSG {} :{}", message.params[0], value));
            }
        }

    }

    fn mainloop(&mut self) {
        { // I <3 the borrow checker.
            let nickname = self.nickname.clone();

            self.send_line(&format!("NICK {}", nickname));
            self.send_line(&format!("USER {} * * :{}", nickname, nickname));
        }

        loop {
            for line in self.read_lines() {
                if line.len() == 0 { break; }
                let message = IrcMessage::new(&line[..]);

                match &message.command[..] {
                    "PING" => self.send_line(&line.replace("PING", "PONG")),
                    "001" => {
                        for channel in self.channels.clone().iter() {
                            self.send_line(&format!("JOIN {}", channel));
                        }
                    },
                    "PRIVMSG" => self.handle_privmsg(message),
                    _ => continue,
                }
            }
        }
    }

    fn from_config(filename: &str) -> Self {
        let mut config_file = File::open(filename).expect("Could not open config.");
        let mut config_buffer = String::new();
        config_file.read_to_string(&mut config_buffer).unwrap();
        let config = config_buffer.parse::<Value>().unwrap();

        // TODO: Refactor this code to not be a nightmare.
        let fetch_string = |key, prompt| String::from(config[key].as_str().expect(prompt));

        let nickname = fetch_string("nickname", "Invalid nickname.");

        let channels = config["channels"].as_array()
                                         .expect("Invalid channels.")
                                         .iter()
                                         .map(|v| String::from(v.as_str().expect("Invalid channels.")))
                                         .collect();

        let host = fetch_string("host", "Invalid host.");

        let port = config["port"].as_integer().expect("Invalid port.");

        OxygenBot::new(nickname, channels, &host, port as u16)
    }
}

fn main() {
    OxygenBot::from_config("oxygen_config.toml").mainloop();
}
