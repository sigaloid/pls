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
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::too_many_lines)]
use std::{fs::DirBuilder, process::Stdio, str::from_utf8};

use clap::{arg, ArgAction, Command};
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
    // create config directory
    create_dir();
    // create path to config file
    let path = ProjectDirs::from("com", "sigaloid", "please")
        .expect("Failed to create ProjectDirs!")
        .config_dir()
        .join("please.json");
    // create database
    let mut db = PickleDb::load_or_new(
        path,
        PickleDbDumpPolicy::AutoDump,
        SerializationMethod::Json,
    );

    // if name has not been set, ask for name and save it
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

    // if weather has not been set, ask whether (ha) to display it.
    if !db.exists("weather") {
        let weather = casual::confirm(
            Paint::blue("Would you like to display the weather based on your IP location each time you open the terminal?")
                .to_string(),
        );
        db.set("weather", &weather)
            .expect("Failed to write weather to database");
        if casual::confirm(
            Paint::cyan("Would you like to save a more specific location (your exact city)?")
                .to_string(),
        ) {
            let city: String = casual::prompt(Paint::blue("Enter a city name: ").to_string()).get();
            db.set("weather-city", &city)
                .expect("Failed to write city to database");
        }
    }
    let matches = clap::Command::new("please").version("0.1.0")
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
            Command::new("undo")
                .about("Mark task as undone")
                .arg(arg!([INDEX])),
        )
        .subcommand(
            Command::new("rm")
                .alias("remove")
                .alias("del")
                .alias("delete")
                .about("Remove task")
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
                .about("Install into shell. \"fish\", \"bash\", or \"zsh\" as options are supported. 
                Alternatively, enter \"weather\" as an option to install a crontab service to automatically update the weather in the background.")
                .arg(arg!([SHELL])),
        )
        .subcommand(Command::new("clean").about("Clean all completed tasks"))
        .arg(
            arg!(
                -r --refresh "Force refresh of weather"
            )
            .action(ArgAction::SetTrue),
        ).arg(
            arg!(
                -a --all "Apply change to all tasks"
              )
            .action(ArgAction::SetTrue),
        )
        .get_matches();
    let all = *matches.get_one::<bool>("all").unwrap_or(&false);
    let force_refresh = *matches.get_one::<bool>("refresh").unwrap_or(&false);
    match matches.subcommand() {
        Some(("add", sub_matches)) => {
            // if name of task is set, add task to list
            if let Some(name) = sub_matches.get_one::<String>("NAME") {
                println!("Adding task {} to list...", Paint::yellow(name));
                // get copy of tasks, add new task, and save to database
                let mut tasks = get_tasks(&db);
                tasks.push(Task::new(name));
                db.set("tasks", &tasks).expect("Failed to set tasks");
                print_tasks(&mut db, false, force_refresh);
            }
        }
        Some(("do", sub_matches)) => {
            // use specified index or default to first
            if all {
                println!("{}", Paint::red("Marking all tasks as done..."));
                // get copy of tasks, mark as completed, replace task in task list
                let tasks = get_tasks(&db);
                let mut new_tasks = Vec::new();
                for task in tasks {
                    let mut new_task = task.clone();
                    new_task.completed = true;
                    new_tasks.push(new_task);
                }
                // save task list to database
                db.set("tasks", &new_tasks).expect("Failed to set tasks");
            } else {
                let index = sub_matches
                    .get_one::<String>("INDEX")
                    .map_or_else(|| 0, |index| index.parse::<usize>().unwrap_or(0))
                    .saturating_sub(1);

                println!(
                    "Marking task {} from list as done...",
                    Paint::yellow(index + 1)
                );
                // get copy of tasks, mark as completed, replace task in task list
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
                // save task list to database
                db.set("tasks", &tasks).expect("Failed to set tasks");
            }
            print_tasks(&mut db, false, force_refresh);
        }
        Some(("undo", sub_matches)) => {
            if all {
                println!("{}", Paint::red("Marking all tasks as undone..."));
                // get copy of tasks, mark as uncompleted, replace task in task list
                let tasks = get_tasks(&db);
                let mut new_tasks = Vec::new();
                for task in tasks {
                    let mut new_task = task.clone();
                    new_task.completed = false;
                    new_tasks.push(new_task);
                }
                // save task list to database
                db.set("tasks", &new_tasks).expect("Failed to set tasks");
            } else {
                // use specified index or default to first
                let index = sub_matches
                    .get_one::<String>("INDEX")
                    .map_or_else(|| 0, |index| index.parse::<usize>().unwrap_or(0))
                    .saturating_sub(1);

                println!(
                    "Marking task {} from list as undone...",
                    Paint::yellow(index + 1)
                );
                // get copy of tasks, mark as uncompleted, replace task in task list
                let mut tasks = get_tasks(&db);
                match tasks.get_mut(index) {
                    Some(task_mut) => {
                        let mut task = task_mut.clone();
                        task.completed = false;
                        let _replace = std::mem::replace(&mut tasks[index], task);
                    }
                    None => println!(
                        "{}",
                        Paint::red(
                            "Error: task not found. Are you sure a task exists with that number?"
                        )
                    ),
                }
                // save task list to database
                db.set("tasks", &tasks).expect("Failed to set tasks");
            }
            print_tasks(&mut db, false, force_refresh);
        }
        Some(("rm", sub_matches)) => {
            if all {
                println!("{}", Paint::red("Removing all tasks..."));
                db.rem("tasks").expect("Failed to remove tasks");
            } else {
                // use specified index or default to first
                let index = sub_matches
                    .get_one::<String>("INDEX")
                    .map_or_else(|| 0, |index| index.parse::<usize>().unwrap_or(0))
                    .saturating_sub(1);

                println!("Removing task {}...", Paint::yellow(index + 1));
                // get copy of tasks, delete from list
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
                // save task list to database
                db.set("tasks", &tasks).expect("Failed to set tasks");
            }
            print_tasks(&mut db, false, force_refresh);
        }
        Some(("install", sub_matches)) => {
            if cfg!(unix) {
                let command = |cmd: &str, shell_install: bool| {
                    println!("Now running command: {}", cmd);
                    let output = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(cmd)
                        .output()
                        .expect("failed to execute process");

                    println!("{}", from_utf8(&output.stdout).unwrap_or_default());

                    if output.status.success() {
                        println!(
                            "Seems like the command was successful. {}",
                            if shell_install {
                                "If not, you can manually add 'please' to your bashrc, zshrc, or fishrc."
                            } else {
                                "If not, you can manually add background weather refresh to your crontab by running 'crontab -e' in a terminal and adding '0 * * * * please -r'"
                            }
                        );
                    }
                    println!("Successfully ran command");
                };
                let install = |path| command(&format!("echo \"please\" >> {}", path), true);
                // if shell is specified, attempt to add "please" to the *rc so that please runs automatically on every shell start.
                if let Some(index) = sub_matches.get_one::<String>("SHELL") {
                    match index.as_str() {
                        "fish" => install("~/.config/fish/config.fish"),
                        "bash" => install("~/.bashrc"),
                        "zsh" => install("~/.zshrc"),
                        // if user specified weather, add a weather refresh to the crontab so that it refreshes the weather 
                        // every 60 minutes and on boot. this ensures that the user never waits for their terminal.
                        "weather" => command(
                            "crontab -l | { cat; echo \"0 * * * * please -r\"; echo \"@reboot please -r\"; } | sort | uniq | crontab -",false
                        ),
                        _ => println!("Must be fish, bash, zsh, or weather (to install the weather background update service)!"),
                    }
                }
            } else {
                println!("Installing to shell is only supported on Linux!");
            }
        }
        Some(("clean", _)) => {
            println!("{}", Paint::blue("Clearing all completed tasks"));
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
            || {
                format!(
                    "{}, {}! It is {}",
                    greeting_gen,
                    time_greeting,
                    time.format(&Rfc2822).unwrap_or_else(|_| time.to_string())
                )
            },
            |name| {
                format!(
                    "{}, {}, {}! It is {}",
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
    let tasks = get_tasks(db);
    let total_task_count = tasks.len();
    let task_pending_count = tasks.iter().filter(|t| !t.completed).count();
    let task_completed_count = tasks.iter().filter(|t| t.completed).count();
    let mut vec = vec![];
    if total_task_count != 0 {
        vec.push(TableCell::new(""));
    }
    vec.extend(vec![TableCell::new_with_alignment(
        format!(
            "You have {} pending tasks and {} completed tasks!",
            Paint::red(task_pending_count),
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
            TableCell::new_with_alignment(Paint::green("#").bold().italic(), 1, Alignment::Center),
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
                TableCell::new_with_alignment(Paint::green(&task.title), 1, Alignment::Center),
                TableCell::new_with_alignment(
                    if task.completed {
                        Paint::green("✅ | Completed!").to_string()
                    } else {
                        Paint::red("❌ | Uncompleted!").to_string()
                    },
                    1,
                    Alignment::Center,
                ),
            ]));
        }
    }

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

fn get_tasks(db: &PickleDb) -> Vec<Task> {
    db.get::<Vec<Task>>("tasks").unwrap_or_default()
}

fn create_dir() {
    if let Some(dir) = ProjectDirs::from("com", "sigaloid", "please") {
        let cfg_dir = dir.config_dir();
        if !cfg_dir.exists() {
            DirBuilder::new().recursive(true).create(cfg_dir).ok();
        }
    }
}
fn get_weather(db: &mut PickleDb, force_refresh: bool) -> Option<String> {
    let timestamp_current = get_time().unix_timestamp();
    let fetch_and_cache_weather = |db: &mut PickleDb| -> Option<String> {
        let city = db.get::<String>("weather-city").unwrap_or_default();
        let get = ureq::get(&format!("https://wttr.in/{}?format=%l:+%C+%c+%t", city))
            .call()
            .ok()?
            .into_string()
            .ok()?;
        db.set("weather-cached", &get)
            .expect("Failed to set cached weather");
        db.set("weather-timestamp", &timestamp_current)
            .expect("Failed to set cached weather");
        Some(get)
    };

    if let Some(timestamp) = db.get::<i64>("weather-timestamp") {
        // if manually forcing a refresh
        if force_refresh {
            // force refresh and block thread when forced
            fetch_and_cache_weather(db)
        } else if timestamp_current - timestamp > 3600 || !db.exists("weather-cached") {
            // if refresh isn't forced, but it is outdated or a cache doesn't exist,
            // spawn new process to update in the background, so that the terminal isn't blocked by a weather update
            drop(
                std::process::Command::new("please")
                    .arg("-r")
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn(),
            );
            // then report a cached version (and if there is none, just use an empty string. The next time it will contain actual weather)
            Some(
                db.get::<String>("weather-cached")
                    .map(|s| format!("{} ({}m outdated)", s, (timestamp_current - timestamp) / 60))
                    .unwrap_or_default(),
            )
        } else {
            // if the timestamp is not outdated simply load cached weather
            db.get::<String>("weather-cached")
        }
    } else {
        fetch_and_cache_weather(db)
    }
}
pub(crate) fn get_time() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}
