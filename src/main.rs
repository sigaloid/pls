use std::io::stdin;

use clap::{arg, command, Command};
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
mod db;
mod quotes;

fn main() {
    db::create_dir();
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
        println!("{}", Paint::blue("Hello! What can I call you?: "));
        let mut buffer = String::new();
        let stdin = stdin();
        stdin.read_line(&mut buffer).expect("Failed to read line");
        println!(
            "Nice to meet you, {}! I'll write that down for you to remember.",
            buffer.trim()
        );
        db.set("name", &buffer.trim())
            .expect("Failed to write name to database");
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
        .subcommand(Command::new("clean").about("Clean all completed tasks"))
        .get_matches();
    match matches.subcommand() {
        Some(("add", sub_matches)) => {
            if let Some(name) = sub_matches.get_one::<String>("NAME") {
                println!("Adding task {} to list...", Paint::yellow(name));
                let mut tasks = get_tasks(&db);
                tasks.push(Task::new(name));
                db.set("tasks", &tasks).expect("Failed to set tasks");
                print_tasks(&mut db, false);
            }
        }
        Some(("do", sub_matches)) => {
            if let Some(index) = sub_matches.get_one::<String>("INDEX") {
                let mut index = index.parse::<usize>().unwrap_or(0);
                if index != 0 {
                    index -= 1;
                }
                println!("Marking task {} from list as done...", Paint::yellow(index));
                let mut tasks = get_tasks(&db);
                match tasks.get_mut(index) {
                    Some(task_mut) => {
                        let mut task = task_mut.clone();
                        task.completed = true;
                        let _ = std::mem::replace(&mut tasks[index], task);
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
                print_tasks(&mut db, false);
            }
        }
        Some(("rm", sub_matches)) => {
            if let Some(index) = sub_matches.get_one::<String>("INDEX") {
                let mut index = index.parse::<usize>().unwrap_or(0);
                if index != 0 {
                    index -= 1;
                }
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
                    )
                }

                db.set("tasks", &tasks).expect("Failed to set tasks");
                print_tasks(&mut db, false);
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
            print_tasks(&mut db, false);
        }
        _ => {
            print_tasks(&mut db, true);
        }
    }
}

fn print_tasks(db: &mut PickleDb, full_greet: bool) {
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
        let full_greeting = if let Some(name) = db.get::<String>("name") {
            format!(
                "{}, {} {}! It is {}",
                greeting_gen,
                time_greeting,
                name,
                time.format(&Rfc2822).unwrap_or(time.to_string())
            )
        } else {
            format!("{}!", greeting_gen)
        };

        let quote = quotes::get_quote(db);
        println!("{}", Paint::yellow(quote));
        println!();
        println!("{}", Paint::green(full_greeting));
        println!();
    }
    if let Some(tasks) = db.get::<Vec<Task>>("tasks") {
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
        if total_task_count != 0 {
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
                    Paint::yellow("Completed").bold().italic(),
                    1,
                    Alignment::Center,
                ),
            ]));
            for (i, task) in tasks.iter().enumerate() {
                table.add_row(Row::new(vec![
                    TableCell::new_with_alignment(Paint::green(i + 1), 1, Alignment::Center),
                    TableCell::new_with_alignment(Paint::green(&task.title), 1, Alignment::Center),
                    TableCell::new_with_alignment(func(task.completed), 1, Alignment::Center),
                ]));
            }
        } else {
            table.add_row(Row::new(vec![TableCell::new_with_alignment(
                Paint::green("Congrats! You are up to date!"),
                2,
                Alignment::Center,
            )]));
        }
    } else {
        println!("{}", Paint::green("No tasks!"));
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

fn get_time() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or(OffsetDateTime::now_utc())
}
