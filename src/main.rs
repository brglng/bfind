use std::env;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::path::Path;
use std::process::exit;

fn depth_first_traverse(prog: &str, root: &Path, allow_hidden: bool, follow_links: bool, iter_depth: i32, depth: i32, ignores: &HashSet<String>) -> bool {
    let mut has_next_level = false;
    let entries = fs::read_dir(root);
    if let Ok(entries) = entries {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if let Some(file_name) = path.file_name() {
                    if let Some(file_name_str) = file_name.to_str() {
                        if let Some(first_char) = file_name_str.chars().next() {
                            let is_hidden = first_char == '.';
                            let is_link = path.read_link().is_ok();
                            if (follow_links || !is_link) && (allow_hidden || !is_hidden) && ignores.get(file_name_str).is_none() {
                                if depth < iter_depth {
                                    if path.is_dir() && depth_first_traverse(prog, &entry.path(), allow_hidden, follow_links, iter_depth, depth + 1, ignores) {
                                        has_next_level = true;
                                    }
                                } else {
                                    if path.is_dir() {
                                        has_next_level = true;
                                    }
                                    println!("{}", path.display());
                                }
                            }
                        }
                    } else {
                        eprintln!("{}: {}: cannot read filename", prog, path.display());
                    }
                } else {
                    eprintln!("{}: {}: cannot read filename", prog, path.display());
                }
            } else if let Err(e) = entry {
                eprintln!("{}: {}: {}", prog, root.display(), e);
            }
        }
    } else if let Err(e) = entries {
        eprintln!("{}: {}: {}", prog, root.display(), e);
    }
    return has_next_level;
}

fn iterative_deepening(prog: &str, mut roots: Vec<String>, allow_dot: bool, follow_links: bool, max_depth: i32, ignores: HashSet<String>) {
    if roots.is_empty() {
        for depth in 1..=max_depth {
            if !depth_first_traverse(prog, Path::new("."), allow_dot, follow_links, depth, 1, &ignores) {
                break;
            }
        }
    } else {
        for depth in 1..=max_depth {
            let mut i = 0_usize;
            while i < roots.len() {
                if !depth_first_traverse(prog, Path::new(&roots[i]), allow_dot, follow_links, depth, 1, &ignores) {
                    roots.remove(i);
                    if roots.is_empty() {
                        return;
                    } else {
                        continue;
                    }
                }
                i += 1;
            }
        }
    }
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

    iterative_deepening(prog, roots, allow_dot, follow_links, max_depth, ignores);
}
