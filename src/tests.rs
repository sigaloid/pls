#![cfg(test)]
use crate::{get_tasks, quotes::get_quote, weather::get_weather, Task};
use pickledb::PickleDb;
use sealed_test::prelude::*;

#[test]
#[sealed_test]
fn test_quotes_and_weather() {
    let mut db = PickleDb::load_or_new(
        "test",
        pickledb::PickleDbDumpPolicy::NeverDump,
        pickledb::SerializationMethod::Json,
    );
    println!("{:?}", get_weather(&mut db, true));

    for i in 0..5000 {
        println!("{i}: {}", get_quote(&mut db));
        println!("{i}: {:?}", get_weather(&mut db, false));
    }
}

#[test]
#[sealed_test]
fn test_tasks() {
    let mut db = PickleDb::load_or_new(
        "test",
        pickledb::PickleDbDumpPolicy::NeverDump,
        pickledb::SerializationMethod::Json,
    );
    assert!(get_tasks(&db).is_empty());
    let tasks = vec![Task::new(&"task".into())];

    db.set("tasks", &tasks).unwrap();
    assert_eq!(db.get::<Vec<Task>>("tasks").unwrap(), tasks);
    assert_eq!(get_tasks(&db), tasks);
    assert!(!get_tasks(&db).is_empty())
}
