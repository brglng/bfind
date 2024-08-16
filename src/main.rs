use std::collections::VecDeque;
use std::env;
use std::fs;
use std::io;
use std::cmp::max;
use std::path::Path;
use std::path::PathBuf;
use std::process::exit;
use std::thread;
use thiserror::Error;

mod path_queue;
use path_queue::PathQueue;

#[derive(Error, Debug)]
enum Error {
    #[error("std::io::Error {{ kind = {} }}: {source}", source.kind())]
    Io {
        #[from]
        source: io::Error
    },

    #[error("path_queue::Error: {source}")]
    PathQueue {
        #[from]
        source: path_queue::Error
    }
}

type Result<T> = std::result::Result<T, Error>;

fn breadth_first_traverse(prog: &str, allow_hidden: bool, follow_links: bool, ignores: &[String], in_queue: &PathQueue, out_queue: &PathQueue) -> Result<()> {
    loop {
        let path = in_queue.pop_timeout(100)?;
        if let Some(path) = path {
            let entries = fs::read_dir(&path);
            if let Ok(entries) = entries {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let metadata = entry.metadata()?;
                        let mut opt_path = None;
                        if follow_links && metadata.is_symlink() {
                            let p = entry.path().read_link();
                            if let Ok(p) = p {
                                opt_path = Some(p);
                            } else {
                                eprintln!("{}: {}: {}", prog, path.display(), p.unwrap_err());
                                continue;
                            }
                        }
                        if let Some(file_name) = entry.file_name().to_str() {
                            if !allow_hidden {
                                if let Some(first_char) = file_name.chars().next() {
                                    if first_char == '.' {
                                        continue;
                                    }
                                }
                            }
                            if ignores.iter().any(|item| item == file_name) {
                                continue;
                            }
                            if opt_path.is_none() {
                                opt_path = Some(entry.path());
                            }
                            let path = unsafe { opt_path.unwrap_unchecked() };
                            println!("{}", path.display());
                            if path.is_dir() {
                                out_queue.push(path)?;
                            }
                        } else {
                            eprintln!("{}: {}: cannot read filename", prog, path.display());
                        }
                    } else {
                        eprintln!("{}: {}: {}", prog, path.display(), entry.unwrap_err());
                    }
                }
            } else {
                eprintln!("{}: {}: {}", prog, path.display(), entries.unwrap_err());
            }
        } else {
            break;
        }
    }

    Ok(())
}

#[derive(PartialEq, Eq)]
enum Verb {
    Print,
    Exec
}

#[derive(PartialEq, Eq)]
enum CliState {
    Options,
    Action,
    Expr,
}

fn print_help(prog: &str) {
    println!("{}: [-H] [-L] [-d DEPTH] [-I IGNORE] [DIR ...] [VERB ...] [-- EXPR ...]", prog);
    exit(0);
}

fn main() {
    let mut args: VecDeque<String> = env::args().collect();

    let mut allow_hidden = false;
    let mut follow_links = false;
    let mut roots: Vec<String> = Vec::new();
    let mut max_depth = i32::MAX;
    let mut ignores: Vec<String> = Vec::new();
    let mut state = CliState::Options;
    let mut verb = Verb::Print;
    let mut action_tokens = Vec::new();
    let mut expr_tokens: Vec<String> = Vec::new();

    let prog_path = args.pop_front().unwrap();
    let prog = prog_path.rsplit('/').nth_back(0).unwrap();

    while let Some(arg) = args.pop_front() {
        match state {
            CliState::Options => {
                if arg == "-H" {
                    allow_hidden = true;
                } else if arg == "-L" {
                    follow_links = true;
                } else if arg == "-h" || arg == "--help" {
                    print_help(prog);
                } else if arg == "-d" || arg == "--depth" {
                    if let Some(depth_str) = args.pop_front() {
                        if let Ok(depth) = depth_str.parse::<i32>() {
                            if depth < 1 {
                                eprintln!("{}: depth must be > 0", prog);
                                exit(1);
                            }
                            max_depth = depth;
                        } else {
                            eprintln!("{}: unable to parse \"{}\" as i32", prog, &depth_str);
                            exit(1);
                        }
                    } else {
                        eprintln!("{}: missing argument to -d", prog);
                        exit(1);
                    }
                } else if arg == "-I" || arg == "--ignore" {
                    if let Some(ignore) = args.pop_front() {
                        ignores = ignore.split(',').map(|s| { s.to_string() }).collect();
                    } else {
                        eprintln!("{}: missing argument to -I", prog);
                        exit(1);
                    }
                } else if arg == "print" {
                    verb = Verb::Print;
                    state = CliState::Action;
                } else if arg == "exec" {
                    verb = Verb::Exec;
                    state = CliState::Action;
                } else if arg == "--" {
                    state = CliState::Expr;
                } else if arg.starts_with('-') {
                    eprintln!("{}: unrecognized argument: {}", prog, &arg);
                    exit(1);
                } else {
                    roots.push(arg);
                }
            },
            CliState::Action => {
                if arg == "--" {
                    state = CliState::Expr;
                } else {
                    action_tokens.push(arg);
                }
            },
            CliState::Expr => {
                expr_tokens.push(arg);
            },
        }
    }

    let num_threads = {
        if let Ok(n) = thread::available_parallelism() {
            max(n.get() , 16)
        } else {
            exit(1);
        }
    };
    let mut queues = Vec::new();
    for _ in 0..num_threads {
        let queue = PathQueue::new((1024 * 512 / num_threads) as u32, (1024 * 512 / num_threads) as u32);
        if let Ok(queue) = queue {
            queues.push(queue);
        } else {
            let e = queue.unwrap_err();
            eprintln!("{}: {}", prog, e);
            exit(1);
        }
    }

    if roots.is_empty() {
        if let Err(e) = queues[0].push(PathBuf::from(".")) {
            eprintln!("{}: {}", prog, e);
            exit(1);
        }
    } else {
        let dotdir = Path::new(".");
        let dotdotdir = Path::new("..");
        let rootdir = Path::new("/");
        for root in &roots {
            let path = PathBuf::from(root);
            if !follow_links && path.is_symlink() {
                continue;
            }
            if path != dotdir && path != dotdotdir && path != rootdir {
                eprintln!("{}", path.display());
                if let Some(file_name) = path.file_name() {
                    if let Some(file_name) = file_name.to_str() {
                        if !allow_hidden {
                            if let Some(first_char) = file_name.chars().next() {
                                if first_char == '.' {
                                    continue;
                                }
                            }
                        }
                        if ignores.iter().any(|item| item == file_name) {
                            continue;
                        }
                    } else {
                        eprintln!("{}: {}: cannot read filename", prog, path.display())
                    }
                } else {
                    unreachable!("path ends with \"..\", which should not happen");
                }
            }
            if let Err(e) = queues[0].push(path) {
                eprintln!("{}: {}", prog, e);
                exit(1)
            }
        }
    }

    if let Err(e) = thread::scope(|s| -> Result<()> {
        let ignores = &ignores;
        let queues = &queues;
        for i in 0..num_threads {
            s.spawn(move|| -> Result<()> {
                breadth_first_traverse(
                    prog, allow_hidden, follow_links, ignores,
                    &queues[i],
                    &queues[(i + 1) % num_threads])
            });
        }
        Ok(())
    }) {
        eprintln!("{}: {}", prog, e);
    }
}
