use std::env;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::exit;
use std::thread;
use thiserror::Error;

mod path_queue;
use path_queue::PathQueue;

#[derive(Error, Debug)]
enum Error {
    #[error("path_queue::Error: {source}")]
    PathQueue {
        #[from]
        source: path_queue::Error
    }
}

type Result<T> = std::result::Result<T, Error>;

fn f(v: &i32) {

}

fn breadth_first_traverse(prog: &str, allow_hidden: bool, follow_links: bool, ignores: &HashSet<String>, in_queue: &PathQueue, out_queue: &PathQueue) -> Result<()> {
    loop {
        let path = in_queue.pop_timeout(200)?;
        if let Some(path) = path {
            let entries = fs::read_dir(&path);
            if let Ok(entries) = entries {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let mut path = entry.path();
                        if follow_links && path.is_symlink() {
                            let p = path.read_link();
                            if let Ok(p) = p {
                                path = p;
                            } else {
                                eprintln!("{}: {}: {}", prog, path.display(), p.unwrap_err());
                                continue;
                            }
                        }
                        if let Some(file_name) = path.file_name() {
                            if let Some(file_name) = file_name.to_str() {
                                if !allow_hidden {
                                    if let Some(first_char) = file_name.chars().next() {
                                        if first_char == '.' {
                                            continue;
                                        }
                                    }
                                }
                                if ignores.get(file_name).is_some() {
                                    continue;
                                }
                                println!("{}", path.display());
                                if path.is_dir() {
                                    out_queue.push(path)?;
                                }
                            } else {
                                eprintln!("{}: {}: cannot read filename", prog, path.display());
                            }
                        } else {
                            unreachable!("path ends with \"..\", which should not happen");
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
    let mut ignores: HashSet<String> = HashSet::new();
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

    let num_threads = 4;
    let mut queues = Vec::new();
    for _ in 0..num_threads {
        let queue = PathQueue::new(1024 * 512 / num_threads, 1024 * 512 / num_threads);
        if let Ok(queue) = queue {
            queues.push(queue);
        } else {
            let e = queue.unwrap_err();
            eprintln!("{}: {}", prog, e);
            return;
        }
    }

    if roots.is_empty() {
        if let Err(e) = queues[0].push(PathBuf::from(".")) {
            eprintln!("{}: {}", prog, e);
            return;
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
                        if ignores.contains(file_name) {
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
                return;
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
                    &queues[i as usize],
                    &queues[((i + 1) % num_threads) as usize])
            });
        }
        Ok(())
    }) {
        eprintln!("{}: {}", prog, e);
    }
}
