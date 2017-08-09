use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::io::Write;
use std::thread;
use std::sync::{Arc, Mutex};

macro_rules! println_stderr(
    ($($arg:tt)*) => { {
        match writeln!(&mut ::std::io::stderr(), $($arg)*) { Ok(_) => {}, Err(_) => {} }
    } }
);

fn worker_walk_dirs(start_dir: &str, all_files: Arc<Mutex<VecDeque<PathBuf>>>) {
    let mut q = VecDeque::new();

    q.push_back(PathBuf::from(start_dir));

    while let Some(file) = q.pop_front() {
        let _ = all_files.lock().map(|mut all_files| {
            all_files.push_back(file.clone());
            if file.is_dir() {
                let _ = fs::read_dir(&file).map(|entries| {
                    for entry in entries {
                        let _ = entry.map(|entry| {
                            q.push_back(entry.path());
                        }).map_err(|err| {
                            println_stderr!("bfind: {}", err);
                        });
                    }
                }).map_err(|err| {
                    println_stderr!("bfind: {}: {}", file.display(), err);
                });
            }
        }).map_err(|err| {
            println_stderr!("bfind: {}", err);
        });
    }

    let _ = all_files.lock().map(|mut all_files| {
        // sign to stop loop
        all_files.push_back(PathBuf::from(""));
    }).map_err(|err| {
        println_stderr!("bfind: {}", err);
    });
}

fn worker_filter(all_files: Arc<Mutex<VecDeque<PathBuf>>>, results: Arc<Mutex<VecDeque<PathBuf>>>) {
    let mut done = false;
    while !done {
        let _ = all_files.lock().map(|mut all_files| {
            if let Some(file) = all_files.pop_front() {
                if file.as_os_str().is_empty() {
                    let _ = results.lock().map(|mut results| {
                        // sign to stop loop
                        results.push_back(PathBuf::from(""));
                    }).map_err(|err| {
                        println_stderr!("bfind: {}", err);
                    });
                    done = true;
                } else {
                    let _ = results.lock().map(|mut results|{
                        results.push_back(file);
                    }).map_err(|err| {
                        println_stderr!("bfind: {}", err);
                    });
                }
            }
        }).map_err(|err| {
            println_stderr!("bfind: {}", err);
        });
    }
}

fn worker_print_results(results: Arc<Mutex<VecDeque<PathBuf>>>) {
    let mut done = false;
    while !done {
        let _ = results.lock().map(|mut results| {
            if let Some(file) = results.pop_front() {
                if file.as_os_str().is_empty() {
                    done = true;
                } else {
                    println!("{}", &file.display());
                }
            }
        }).map_err(|err| {
            println_stderr!("bfind: {}", err);
        });
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let all_files = Arc::new(Mutex::new(VecDeque::new()));

    let thread_walk_dirs;
    {
        let start_dir = if args.len() > 1 {
            args[1].clone()
        } else {
            String::from(".")
        };

        let all_files = all_files.clone();
        thread_walk_dirs = thread::spawn(move || {
            worker_walk_dirs(&start_dir, all_files);
        });
    }

    let filtered_files = Arc::new(Mutex::new(VecDeque::new()));

    let thread_filter;
    {
        let all_files = all_files.clone();
        let filtered_files = filtered_files.clone();
        thread_filter = thread::spawn(move || {
            worker_filter(all_files, filtered_files);
        });
    }

    let thread_print_results;
    {
        let filtered_files = filtered_files.clone();
        thread_print_results = thread::spawn(move || {
            worker_print_results(filtered_files);
        });
    }

    thread_walk_dirs.join().expect("failed to join thread thread_walk_dirs");
    thread_filter.join().expect("failed to join thread thread_filter");
    thread_print_results.join().expect("failed to join thread thread_print_results");
}
