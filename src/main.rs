#![deny(
    anonymous_parameters,
    clippy::all,
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

use clap::{arg, ArgAction, Command};
use directories_next::ProjectDirs;
use native_tls::TlsConnector;
use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};
use serde::{Deserialize, Serialize};
use spinach::{term, Spinach};
use std::{fs::DirBuilder, str::from_utf8, sync::Arc};
use tegen::tegen::TextGenerator;
use term_table::{
    row::Row,
    table_cell::{Alignment, TableCell},
    TableStyle,
};
use time::{macros::format_description, OffsetDateTime};
use ureq::Response;
use yansi::Paint;

use crate::weather::get_weather;
mod quotes;
mod tests;
mod weather;
fn main() {
    // https://github.com/etienne-napoleone/spinach#how-to-avoid-leaving-terminal-without-prompt-on-interupt-ctrlc
    ctrlc::set_handler(|| {
        term::show_cursor();
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    // create config directory
    create_dir();

    // create path to config file
    let path = ProjectDirs::from("com", "sigaloid", "pls")
        .expect("Failed to create ProjectDirs!")
        .config_dir()
        .join("pls.json");

    // create database
    let mut db = PickleDb::load_or_new(
        path,
        PickleDbDumpPolicy::AutoDump,
        SerializationMethod::Json,
    )
    .expect("Failed to create database!");

    // if name has not been set, ask for name and save it
    if !db.exists("name") {
        let name: String =
            casual::prompt(Paint::blue("Hello! What can I call you?: ").to_string()).get();
        println!(
            "{}",
            Paint::green(&format!(
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
        // set weather key to location
        db.set("weather", &weather)
            .expect("Failed to write weather to database");
        // if user requested to check basic weather, ask if they want to add a specific location
        if weather {
            let s = Spinach::new("Checking your weather...");
            // format weather as *just* the location
            let connector = TlsConnector::new().unwrap();
            let agent = ureq::AgentBuilder::new().tls_connector(Arc::new(connector)).build();
            let current_location = agent.get("https://wttr.in/?format=%l")
                .call()
                .ok()
                .unwrap_or_else(|| Response::new(301, "", "").unwrap())
                .into_string()
                .unwrap_or_default();
            s.succeed("Weather retrieved");
            println!(
                "Your estimated location is: {}. If this is incorrect, you can save a more specific location now.",
                Paint::yellow(&current_location)
            );
            if casual::confirm(
                Paint::cyan(
                    "Would you like to save a more specific location (ex: your exact city)?",
                )
                .to_string(),
            ) {
                let specific_location: String =
                    casual::prompt(Paint::blue("Enter a more specific location: ").to_string())
                        .get();
                // set more specific location
                db.set("weather-specific-location", &specific_location)
                    .expect("Failed to write specific-location to database");
            }
        }
    }
    let matches = clap::Command::new("pls").version("0.1.0")
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
            Command::new("undo").alias("undone")
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
        ).arg(
            arg!(
                -w --weather "Just print the weather!"
            ).action(ArgAction::SetTrue)
        )
        .get_matches();
    // bool that represents whether the command should apply changes to all tasks
    let all = *matches.get_one::<bool>("all").unwrap_or(&false);
    // bool that represents whether the weather should be refreshed
    let force_refresh = *matches.get_one::<bool>("refresh").unwrap_or(&false);
    // bool that represents just the weather to be printed
    let weather = *matches.get_one::<bool>("weather").unwrap_or(&false);
    // match each subcommand
    match matches.subcommand() {
        Some(("add", sub_matches)) => {
            // if name of task is set, add task to list; if not, prompt user
            let task = sub_matches.get_one::<String>("NAME").map_or_else(
                || casual::prompt("Enter task: ").get(),
                std::borrow::ToOwned::to_owned,
            );
            println!("Adding task {} to list...", Paint::yellow(&task));
            // get copy of tasks, add new task, and save to database
            let mut tasks = get_tasks(&db);
            tasks.push(Task::new(&task));
            db.set("tasks", &tasks).expect("Failed to set tasks");
            print_tasks(&mut db, false, force_refresh, false);
        }
        Some(("do", sub_matches)) => {
            // use specified index or default to first
            if all {
                println!("{}", Paint::red("Marking all tasks as done..."));
                // get copy of tasks, mark as completed, replace task in task list
                let new_tasks = get_tasks(&db)
                    .into_iter()
                    .map(|x| make_complete(&x))
                    .collect::<Vec<Task>>();

                // save task list to database
                db.set("tasks", &new_tasks).expect("Failed to set tasks");
            } else {
                let index = sub_matches
                    .get_one::<String>("INDEX")
                    .map_or_else(|| 0, |index| index.parse::<usize>().unwrap_or(0))
                    .saturating_sub(1);

                println!(
                    "Marking task {} from list as done...",
                    Paint::yellow(&(index + 1))
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
            print_tasks(&mut db, false, force_refresh, false);
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
                    Paint::yellow(&(index + 1))
                );
                // get copy of tasks, mark as uncompleted, replace task in task list
                let mut tasks = get_tasks(&db);
                match tasks.get_mut(index) {
                    Some(task_mut) => {
                        // set task as completed and replace it in task list
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
            print_tasks(&mut db, false, force_refresh, false);
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

                println!("Removing task {}...", Paint::yellow(&(index + 1)));
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
            print_tasks(&mut db, false, force_refresh, false);
        }
        Some(("install", sub_matches)) => {
            // code to manage installing to shell
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
                                "If not, you can manually add 'pls' to your bashrc, zshrc, or fishrc."
                            } else {
                                "If not, you can manually add background weather refresh to your crontab by running 'crontab -e' in a terminal and adding '0 * * * * pls -r'"
                            }
                        );
                    }
                    println!("Successfully ran command");
                };
                let install = |path| command(&format!("echo \"pls\" >> {}", path), true);
                // if shell is specified, attempt to add "pls" to the *rc so that pls runs automatically on every shell start.
                if let Some(index) = sub_matches.get_one::<String>("SHELL") {
                    match index.as_str() {
                        "fish" => install("~/.config/fish/config.fish"),
                        "bash" => install("~/.bashrc"),
                        "zsh" => install("~/.zshrc"),
                        // if user specified weather, add a weather refresh to
                        // the crontab so that it refreshes the weather every
                        // 15 minutes and on boot. this ensures that the user 
                        // never waits for their terminal and always has updated
                        // weather
                        "weather" => command(
                            "crontab -l | { cat; echo \"*/15 * * * * pls -r\"; echo \"@reboot pls -r\"; } | sort | uniq | crontab -",false
                        ),
                        _ => println!("Must be fish, bash, zsh, or weather (to install the weather background update service)!"),
                    }
                }
            } else {
                println!("Installing to shell is only supported on Linux!");
            }
        }
        Some(("clean", _)) => {
            // remove all completed tasks
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
                Paint::green(&(prior_len - cleaned_tasks.len()))
            );
            print_tasks(&mut db, false, force_refresh, false);
        }
        Some(("list", _)) => {
            // list all tasks without full greeting
            print_tasks(&mut db, false, force_refresh, false);
        }
        _ => {
            // list all tasks with full greeting
            print_tasks(&mut db, true, force_refresh, weather);
        }
    }
}

fn print_tasks(db: &mut PickleDb, full_greet: bool, force_refresh: bool, just_weather: bool) {
    println!();
    // If just weather
    if just_weather {
        let weather = get_weather(db, force_refresh).unwrap_or_default();
        println!("{}", &weather);
        return;
    }
    let mut table = term_table::Table::new();
    table.style = TableStyle::extended();
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
        let format = format_description!(
            "[hour repr:12]:[minute], [weekday repr:short], [day] [month] [year]"
        );
        let full_greeting = db.get::<String>("name").map_or_else(
            || {
                format!(
                    "{}, {}! It is {}",
                    greeting_gen,
                    time_greeting,
                    time.format(&format).unwrap()
                )
            },
            |name| {
                format!(
                    "{}, {}, {}! It is {}",
                    greeting_gen,
                    time_greeting,
                    name,
                    time.format(&format).unwrap_or_else(|_| time.to_string())
                )
            },
        );

        let quote = quotes::get_quote(db);
        println!("{}\n", Paint::yellow(&quote));
        println!("{}\n", Paint::green(&full_greeting));
        // if weather is enabled
        if db.get::<bool>("weather").unwrap_or_default() {
            get_weather(db, force_refresh).map_or_else(
                |e| println!("{} - {e}", Paint::red(" Failed to fetch weather :(")),
                |weather| {
                    println!("{}\n", Paint::blue(&weather));
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
            Paint::red(&task_pending_count),
            Paint::green(&task_completed_count)
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
                Paint::yellow("Title").bold().italic(),
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
                TableCell::new_with_alignment(Paint::green(&(i + 1)), 1, Alignment::Center),
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

#[derive(Serialize, Deserialize, Clone, Default, PartialEq, Eq, Debug)]
struct Task {
    title: String,
    completed: bool,
}

impl Task {
    fn new(title: &String) -> Self {
        Self {
            title: title.to_string(),
            ..Default::default()
        }
    }
    fn new_completed(title: &String) -> Self {
        Self {
            title: title.to_string(),
            completed: true,
        }
    }
    fn make_complete(&self) -> Self {
        Self::new_completed(&self.title)
    }
}

fn make_complete(task: &Task) -> Task {
    task.make_complete()
}

fn get_tasks(db: &PickleDb) -> Vec<Task> {
    db.get::<Vec<Task>>("tasks").unwrap_or_default()
}

fn create_dir() {
    if let Some(dir) = ProjectDirs::from("com", "sigaloid", "pls") {
        let cfg_dir = dir.config_dir();
        if !cfg_dir.exists() {
            DirBuilder::new().recursive(true).create(cfg_dir).ok();
        }
    }
}
pub(crate) fn get_time() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}
