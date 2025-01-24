// Copyright 2021, 2022 Martin Pool

//! Print messages and progress bars on the terminal.

use std::time::Instant;

use ::console::{style, StyledObject};
use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};

use crate::lab::Scenario;
use crate::mutate::Mutation;
use crate::outcome::{Outcome, Phase};
use crate::*;

/// Top-level UI object that manages the state of an interactive console: mostly progress bars and
/// messages.
pub struct Console {
    show_times: bool,
}

impl Console {
    /// Construct a new rich text UI.
    pub fn new(options: &Options) -> Console {
        Console {
            show_times: options.show_times,
        }
    }

    pub fn start_scenario(&self, scenario: &Scenario) -> Activity {
        match scenario {
            Scenario::SourceTree => self.start_activity("source tree"),
            Scenario::Baseline => self.start_activity("unmutated baseline"),
            Scenario::Mutant {
                mutation,
                i_mutation,
                n_mutations,
            } => {
                let mut activity = self.start_activity(&style_mutation(mutation));
                activity.overall_progress = Some((i_mutation + 1, *n_mutations));
                activity
            }
        }
    }

    /// Start a general-purpose activity.
    pub fn start_activity(&self, task: &str) -> Activity {
        let progress_bar = ProgressBar::new(0)
            .with_message(task.to_owned())
            .with_style(
                ProgressStyle::default_spinner()
                    .template("{msg} ... {elapsed:.cyan} {spinner:.cyan}"),
            );
        progress_bar.set_draw_rate(5); // updates per second
        Activity {
            task: task.to_owned(),
            progress_bar,
            start_time: Instant::now(),
            console: self,
            overall_progress: None,
        }
    }

    /// Start an Activity for copying a tree.
    pub fn start_copy_activity(&self, name: &str) -> CopyActivity {
        CopyActivity::new(name, self)
    }
}

pub struct Activity<'c> {
    pub start_time: Instant,
    progress_bar: ProgressBar,
    task: String,
    console: &'c Console,
    /// Optionally, progress counter through the overall lab. Shown in the progress bar
    /// but not on permanent output.
    overall_progress: Option<(usize, usize)>,
}

impl<'c> Activity<'c> {
    pub fn set_phase(&mut self, phase: &'static str) {
        let overall_text = self
            .overall_progress
            .map_or(String::new(), |(a, b)| format!("[{}/{}] ", a, b));
        self.progress_bar
            .set_message(format!("{}{} ({})", overall_text, self.task, phase));
    }

    /// Mark this activity as interrupted.
    pub fn interrupted(&mut self) {
        self.progress_bar.finish_and_clear();
        println!("{} ... {}", self.task, style("interrupted").bold().red());
    }

    pub fn tick(&mut self) {
        self.progress_bar.tick();
    }

    /// Report the outcome of a scenario.
    ///
    /// Prints the log content if appropriate.
    pub fn outcome(self, outcome: &Outcome, options: &Options) -> Result<()> {
        self.progress_bar.finish_and_clear();
        if (outcome.mutant_caught() && !options.print_caught)
            || (outcome.scenario.is_mutant()
                && outcome.check_or_build_failed()
                && !options.print_unviable)
        {
            return Ok(());
        }

        print!("{} ... {}", self.task, style_outcome(outcome));
        if self.console.show_times {
            println!(" in {}", self.format_elapsed());
        } else {
            println!();
        }
        if outcome.should_show_logs() || options.show_all_logs {
            print!("{}", outcome.get_log_content()?);
        }
        Ok(())
    }

    fn format_elapsed(&self) -> String {
        format_elapsed(self.start_time)
    }
}

pub struct CopyActivity<'c> {
    name: String,
    progress_bar: ProgressBar,
    start_time: Instant,
    console: &'c Console,
}

impl<'c> CopyActivity<'c> {
    fn new(name: &str, console: &'c Console) -> CopyActivity<'c> {
        let progress_bar = ProgressBar::new(0)
            .with_message(name.to_owned())
            .with_style(ProgressStyle::default_spinner().template("{msg}"));
        progress_bar.set_draw_rate(5); // updates per second
        CopyActivity {
            name: name.to_owned(),
            progress_bar,
            start_time: Instant::now(),
            console,
        }
    }

    pub fn bytes_copied(&mut self, bytes_copied: u64) {
        let styled = format!(
            "{} ... {} in {}",
            self.name,
            style_mb(bytes_copied),
            style(format!("{}s", self.start_time.elapsed().as_secs())).cyan(),
        );
        self.progress_bar.set_message(styled);
    }

    pub fn succeed(self, bytes_copied: u64) {
        self.progress_bar.finish_and_clear();
        // Print to stdout even if progress bars weren't drawn.
        print!("{} ...", self.name);
        if self.console.show_times {
            println!(
                " {} in {}",
                style_mb(bytes_copied),
                style(format_elapsed(self.start_time)).cyan(),
            );
        } else {
            println!(" {}", style("done").green());
        }
    }

    pub fn fail(self) {
        self.progress_bar.finish_and_clear();
        println!("{} ... {}", self.name, style("failed").bold().red(),);
    }
}

/// Return a styled string reflecting the moral value of this outcome.
pub fn style_outcome(outcome: &Outcome) -> StyledObject<&'static str> {
    use CargoResult::*;
    use Scenario::*;
    match &outcome.scenario {
        SourceTree | Baseline => match outcome.last_phase_result() {
            Success => style("ok").green(),
            Failure => style("FAILED").red().bold(),
            Timeout => style("TIMEOUT").red().bold(),
        },
        Mutant { .. } => match (outcome.last_phase(), outcome.last_phase_result()) {
            (Phase::Test, Failure) => style("caught").green(),
            (Phase::Test, Success) => style("NOT CAUGHT").red().bold(),
            (Phase::Build, Success) => style("build ok").green(),
            (Phase::Check, Success) => style("check ok").green(),
            (Phase::Build, Failure) => style("build failed").yellow(),
            (Phase::Check, Failure) => style("check failed").yellow(),
            (_, Timeout) => style("TIMEOUT").red().bold(),
        },
    }
}

pub fn list_mutations(mutations: &[Mutation], show_diffs: bool) {
    for mutation in mutations {
        println!("{}", style_mutation(mutation));
        if show_diffs {
            println!("{}", mutation.diff());
        }
    }
}

fn style_mutation(mutation: &Mutation) -> String {
    format!(
        "{}: replace {}{}{} with {}",
        mutation.describe_location(),
        style(mutation.function_name()).bright().magenta(),
        if mutation.return_type().is_empty() {
            ""
        } else {
            " "
        },
        style(mutation.return_type()).magenta(),
        style(mutation.replacement_text()).yellow(),
    )
}

pub fn print_error(msg: &str) {
    println!("{}: {}", style("error").bold().red(), msg);
}

fn format_elapsed(since: Instant) -> String {
    format!("{:.3}s", since.elapsed().as_secs_f64())
}

fn format_mb(bytes: u64) -> String {
    format!("{} MB", bytes / 1_000_000)
}

fn style_mb(bytes: u64) -> StyledObject<String> {
    style(format_mb(bytes)).cyan()
}
