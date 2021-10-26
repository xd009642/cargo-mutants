// Copyright 2021 Martin Pool

//! Print messages to the terminal.

use std::io::{stdout, Write};
use std::time::Instant;

use atty::Stream;
use console::style;

use crate::outcome::{Outcome, Status};

pub(crate) struct Activity {
    pub start_time: Instant,
    atty: bool,
}

impl Activity {
    pub fn start(msg: &str) -> Activity {
        print!("{} ... ", msg);
        stdout().flush().unwrap();
        Activity {
            start_time: Instant::now(),
            atty: atty::is(Stream::Stdout),
        }
    }

    pub fn succeed(self, msg: &str) {
        println!("{} in {}", style(msg).green(), self.format_elapsed());
    }

    pub fn fail(self, msg: &str) {
        println!("{} in {}", style(msg).red().bold(), self.format_elapsed());
    }

    pub fn tick(&self) {
        if self.atty {
            let time_str = format!("{}s", self.start_time.elapsed().as_secs());
            let backspace = "\x08".repeat(time_str.len());
            print!("{}{}", time_str, backspace);
            stdout().flush().unwrap();
        }
    }

    pub fn outcome(self, outcome: &Outcome) {
        match outcome.status {
            Status::Failed => self.succeed("caught"),
            Status::Passed => self.fail("NOT CAUGHT"),
            Status::Timeout => self.fail("TIMEOUT"),
        }
    }

    fn format_elapsed(&self) -> String {
        format!("{:.3}s", &self.start_time.elapsed().as_secs_f64())
    }
}