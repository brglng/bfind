use std::env;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

mod path_queue;
use path_queue::PathQueue;

fn breadth_first_traverse(prog: &str, roots: Vec<String>, allow_hidden: bool, follow_links: bool, ignores: &HashSet<String>) -> path_queue::Result<()> {
    let dotdir = Path::new(".");

    let mut q = PathQueue::new(1024 * 1024, 1024 * 512);

    if roots.is_empty() {
        q.push(PathBuf::from("."))?;
    } else {
        for root in roots {
            q.push(PathBuf::from(root))?;
        }
    }

    while let Ok(path) = q.pop() {
        if !follow_links && path.is_symlink() {
            continue;
        }
        if path != dotdir {
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
                } else {
                    eprintln!("{}: {}: cannot read filename", prog, path.display())
                }
            } else {
                // path ends with ".."
            }
        }

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
                                q.push(path)?;
                            }
                        } else {
                            eprintln!("{}: {}: cannot read filename", prog, path.display());
                        }
                    }
                } else {
                    eprintln!("{}: {}: {}", prog, path.display(), entry.unwrap_err());
                }
            }
        } else {
            eprintln!("{}: {}: {}", prog, path.display(), entries.unwrap_err());
        }

        if q.len() == 0 {
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

    let mut allow_dot = false;
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
                    allow_dot = true;
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

    if let Err(e) = breadth_first_traverse(prog, roots, allow_dot, follow_links, &ignores) {
        eprintln!("{}: {}", prog, e);
    }
}
