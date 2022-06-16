#![deny(
    anonymous_parameters,
    clippy::all,
    const_err,
    illegal_floating_point_literal_pattern,
    late_bound_lifetime_arguments,
    path_statements,
    patterns_in_fns_without_body,
    rust_2018_idioms,
    trivial_numeric_casts,
    unused_extern_crates
)]
#![warn(
    clippy::dbg_macro,
    clippy::decimal_literal_representation,
    clippy::get_unwrap,
    clippy::nursery,
    clippy::pedantic,
    clippy::todo,
    clippy::unimplemented,
    clippy::use_debug,
    clippy::all,
    unused_qualifications,
    variant_size_differences
)]
use std::fs::DirBuilder;

use clap::{arg, command, ArgAction, Command};
use directories_next::ProjectDirs;
use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use serde::{Deserialize, Serialize};
use tegen::tegen::TextGenerator;
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    TableStyle,
};

use time::{format_description::well_known::Rfc2822, OffsetDateTime};
use yansi::Paint;
mod quotes;

fn main() {
    create_dir();
    let path = ProjectDirs::from("com", "sigaloid", "please-rs")
        .expect("Failed to create ProjectDirs!")
        .config_dir()
        .join("please.json");

    let mut db = PickleDb::load_or_new(
        path,
        PickleDbDumpPolicy::AutoDump,
        SerializationMethod::Json,
    );
    if !db.exists("name") {
        let name: String =
            casual::prompt(Paint::blue("Hello! What can I call you?: ").to_string()).get();

        println!(
            "{}",
            Paint::green(format!(
                "Nice to meet you, {}! I'll write that down and make sure I don't forget it.",
                name
            ))
        );
        db.set("name", &name)
            .expect("Failed to write name to database");
    }
    if !db.exists("weather") {
        let weather = casual::confirm(
            Paint::blue("Would you like to display the weather based on your IP location each time you open the terminal?")
                .to_string(),
        );
        db.set("weather", &weather)
            .expect("Failed to write weather to database");
        if casual::confirm(Paint::cyan("Would you like to specify a location?").to_string()) {
            let city: String = casual::prompt(Paint::blue("Enter a city name: ").to_string()).get();
            db.set("weather-city", &city)
                .expect("Failed to write city to database");
        }
    }
    let matches = command!()
        .propagate_version(true)
        .subcommand_required(false)
        .arg_required_else_help(false)
        .subcommand(
            Command::new("add")
                .about("Add task to todo")
                .arg(arg!([NAME])),
        )
        .subcommand(
            Command::new("do")
                .alias("done")
                .about("Mark task as done")
                .arg(arg!([INDEX])),
        )
        .subcommand(
            Command::new("rm")
                .alias("remove")
                .alias("del")
                .alias("delete")
                .about("Mark task as done")
                .arg(arg!([INDEX])),
        )
        .subcommand(
            Command::new("list")
                .alias("ls")
                .alias("all")
                .about("List tasks"),
        )
        .subcommand(
            Command::new("install")
                .about("Install into shell")
                .arg(arg!([SHELL])),
        )
        .subcommand(Command::new("clean").about("Clean all completed tasks"))
        .arg(
            arg!(
                -r --refresh "Force refresh of weather"
            )
            .action(ArgAction::SetTrue),
        )
        .get_matches();
    let force_refresh = *matches.get_one::<bool>("refresh").unwrap_or(&false);
    match matches.subcommand() {
        Some(("add", sub_matches)) => {
            if let Some(name) = sub_matches.get_one::<String>("NAME") {
                println!("Adding task {} to list...", Paint::yellow(name));
                let mut tasks = get_tasks(&db);
                tasks.push(Task::new(name));
                db.set("tasks", &tasks).expect("Failed to set tasks");
                print_tasks(&mut db, false, force_refresh);
            }
        }
        Some(("do", sub_matches)) => {
            if let Some(index) = sub_matches.get_one::<String>("INDEX") {
                let mut index = index.parse::<usize>().unwrap_or(0);
                index = index.saturating_sub(1);
                println!("Marking task {} from list as done...", Paint::yellow(index));
                let mut tasks = get_tasks(&db);
                match tasks.get_mut(index) {
                    Some(task_mut) => {
                        let mut task = task_mut.clone();
                        task.completed = true;
                        let _replace = std::mem::replace(&mut tasks[index], task);
                    }
                    None => println!(
                        "{}",
                        Paint::red(
                            "Error: task not found. Are you sure a task exists with that number?"
                        )
                    ),
                }
                // task.done = true;
                db.set("tasks", &tasks).expect("Failed to set tasks");
                print_tasks(&mut db, false, force_refresh);
            }
        }
        Some(("rm", sub_matches)) => {
            if let Some(index) = sub_matches.get_one::<String>("INDEX") {
                let mut index = index.parse::<usize>().unwrap_or(0);
                index = index.saturating_sub(1);

                println!("Marking task {} from list as done...", Paint::yellow(index));
                let mut tasks = get_tasks(&db);
                if tasks.get(index).is_some() {
                    tasks.remove(index);
                } else {
                    println!(
                        "{}",
                        Paint::red(
                            "Error: task not found. Are you sure a task exists with that number?"
                        )
                    );
                }

                db.set("tasks", &tasks).expect("Failed to set tasks");
                print_tasks(&mut db, false, force_refresh);
            }
        }
        Some(("install", sub_matches)) => {
            if cfg!(linux) {
                let install = |path| {
                    std::process::Command::new("sh")
                        .arg("-c")
                        .arg(format!("echo \"please-rs\" >> {}", path))
                        .output()
                        .expect("failed to execute process");
                };
                if let Some(index) = sub_matches.get_one::<String>("INDEX") {
                    match index.as_str() {
                        "fish" => install("~/.config/fish/config.fish"),
                        "bash" => install("~/.bashrc"),
                        "zsh" => install("~/.zshrc"),
                        _ => {
                            println!("Must be fish, bash, or zsh!");
                        }
                    }
                }
            } else {
                println!("Installing to shell is only supported on Linux!");
            }
        }
        Some(("clean", _)) => {
            let tasks = get_tasks(&db);
            let prior_len = tasks.len();
            let cleaned_tasks = tasks
                .into_iter()
                .filter(|t| !t.completed)
                .collect::<Vec<_>>();
            db.set("tasks", &cleaned_tasks)
                .expect("Failed to set tasks");
            println!(
                "Cleaned {} completed tasks!",
                Paint::green(prior_len - cleaned_tasks.len())
            );
            print_tasks(&mut db, false, force_refresh);
        }
        Some(("list", _)) => {
            print_tasks(&mut db, false, force_refresh);
        }
        _ => {
            print_tasks(&mut db, true, force_refresh);
        }
    }
}

fn print_tasks(db: &mut PickleDb, full_greet: bool, force_refresh: bool) {
    println!();
    let mut table = term_table::Table::new();
    table.style = TableStyle::extended();
    // table.max_column_width = 80;
    if full_greet {
        let time = get_time();
        let time_greeting = match time.hour() {
            5..=12 => "good morning",
            13..=17 => "good afternoon",
            18..=24 | 0..=4 => "good evening",
            _ => "good day",
        };

        let greeting_gen = TextGenerator::new()
            .generate("{Hello|Howdy|Greetings|What's up|Salutations|Greetings}");
        let full_greeting = db.get::<String>("name").map_or_else(
            || format!("{}!", greeting_gen),
            |name| {
                format!(
                    "{}, {} {}! It is {}",
                    greeting_gen,
                    time_greeting,
                    name,
                    time.format(&Rfc2822).unwrap_or_else(|_| time.to_string())
                )
            },
        );

        let quote = quotes::get_quote(db);
        println!("{}", Paint::yellow(quote));
        println!();
        println!("{}", Paint::green(full_greeting));
        println!();
        if db.get::<bool>("weather").unwrap_or_default() {
            get_weather(db, force_refresh).map_or_else(
                || println!("{}", Paint::red("Failed to fetch weather :(")),
                |weather| {
                    println!("{}", Paint::blue(weather));
                    println!();
                },
            );
        }
    }

    db.get::<Vec<Task>>("tasks").map_or_else(
        || {
            println!("{}", Paint::green("No tasks!"));
        },
        |tasks| {
            let total_task_count = tasks.len();
            let task_todo_count = tasks.iter().filter(|t| !t.completed).count();
            let task_completed_count = tasks.iter().filter(|t| t.completed).count();
            let mut vec = vec![];
            if total_task_count != 0 {
                vec.push(TableCell::new(""));
            }
            vec.extend(vec![TableCell::new_with_alignment(
                format!(
                    "You have {} pending tasks and {} completed tasks!",
                    Paint::red(task_todo_count),
                    Paint::green(task_completed_count)
                ),
                2,
                Alignment::Center,
            )]);
            table.add_row(Row::new(vec));
            if total_task_count == 0 {
                table.add_row(Row::new(vec![TableCell::new_with_alignment(
                    Paint::green("Congrats! You are up to date!"),
                    2,
                    Alignment::Center,
                )]));
            } else {
                table.add_row(Row::new(vec![
                    TableCell::new_with_alignment(
                        Paint::green("#").bold().italic(),
                        1,
                        Alignment::Center,
                    ),
                    TableCell::new_with_alignment(
                        Paint::green("Title").bold().italic(),
                        1,
                        Alignment::Center,
                    ),
                    TableCell::new_with_alignment(
                        Paint::yellow("Status").bold().italic(),
                        1,
                        Alignment::Center,
                    ),
                ]));
                for (i, task) in tasks.iter().enumerate() {
                    table.add_row(Row::new(vec![
                        TableCell::new_with_alignment(Paint::green(i + 1), 1, Alignment::Center),
                        TableCell::new_with_alignment(
                            Paint::green(&task.title),
                            1,
                            Alignment::Center,
                        ),
                        TableCell::new_with_alignment(func(task.completed), 1, Alignment::Center),
                    ]));
                }
            }
        },
    );
    println!("{}", table.render());
}

#[derive(Serialize, Deserialize, Clone)]
struct Task {
    title: String,
    completed: bool,
}

impl Task {
    fn new(title: &String) -> Self {
        Self {
            title: title.to_string(),
            completed: false,
        }
    }
}
fn func(o: bool) -> String {
    if o {
        Paint::green("✅ | Completed!").to_string()
    } else {
        Paint::red("❌ | Uncompleted!").to_string()
    }
}

fn get_tasks(db: &PickleDb) -> Vec<Task> {
    db.get::<Vec<Task>>("tasks").unwrap_or_default()
}

fn create_dir() {
    if let Some(dir) = ProjectDirs::from("com", "sigaloid", "please-rs") {
        let cfg_dir = dir.config_dir();
        if !cfg_dir.exists() {
            DirBuilder::new().recursive(true).create(cfg_dir).ok();
        }
    }
}
fn get_weather(db: &mut PickleDb, force_refresh: bool) -> Option<String> {
    let timestamp_current = get_time().unix_timestamp();
    let cache_weather = |db: &mut PickleDb| -> Option<String> {
        let city = db.get::<String>("weather-city").unwrap_or_default();
        let get = ureq::get(&format!("https://wttr.in/{}?format=\"%l:+%C+%c+%t\"", city))
            .call()
            .unwrap()
            .into_string()
            .ok()?
            .replace('"', "");
        db.set("weather-cached", &get)
            .expect("Failed to set cached weather");
        db.set("weather-timestamp", &timestamp_current)
            .expect("Failed to set cached weather");
        Some(get)
    };
    if let Some(timestamp) = db.get::<i64>("weather-timestamp") {
        if timestamp_current - timestamp > 3600 || !db.exists("weather-cached") || force_refresh {
            cache_weather(db)
        } else {
            db.get::<String>("weather-cached")
        }
    } else {
        cache_weather(db)
    }
}
pub(crate) fn get_time() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}
