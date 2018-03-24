use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::thread;

extern crate deque;
use deque::{Worker, Stealer, Stolen};

fn walk_dirs(root_dir: &str, worker: Worker<PathBuf>) {
    let mut q = VecDeque::new();

    q.push_back(PathBuf::from(root_dir));

    while let Some(file) = q.pop_front() {
        if file.is_dir() {
            let _ = fs::read_dir(&file).map(|entries| {
                for entry in entries {
                    let _ = entry.map(|entry| {
                        let entry_path = entry.path();
                        if entry.path().is_dir() {
                            q.push_back(entry_path.clone());
                        }
                        worker.push(entry_path);
                    }).map_err(|err| {
                        eprintln!("bfind: {}", err);
                    });
                }
            }).map_err(|err| {
                eprintln!("bfind: {}: {}", file.display(), err);
            });
        } else {
            // There are some cases in which entry.path().is_dir() above
            // returned true but here file.is_dir() returned false. This often
            // occurs when the file is a symlink to a directory but we don't
            // have access to it. Don't know why though. Just push this path
            // to the Deque.
            worker.push(file);
        }
    }

    worker.push(PathBuf::from(""));
}

fn filter_files(stealer: Stealer<PathBuf>) {
    let mut done = false;
    while !done {
        let stolen = stealer.steal();
        match stolen {
            Stolen::Data(file) => {
                if file.as_os_str().is_empty() {
                    done = true;
                } else {
                    println!("{}", &file.display());
                }
            },
            _ => {}
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let (all_files_worker, all_files_stealer) = deque::new();

    let thread_walk_dirs;
    {
        let root_dir = if args.len() > 1 {
            args[1].clone()
        } else {
            String::from(".")
        };

        thread_walk_dirs = thread::spawn(move || {
            walk_dirs(&root_dir, all_files_worker);
        });
    }

    let thread_filter;
    {
        thread_filter = thread::spawn(move || {
            filter_files(all_files_stealer);
        });
    }

    thread_walk_dirs.join().expect("failed to join thread thread_walk_dirs");
    thread_filter.join().expect("failed to join thread thread_filter");
}
