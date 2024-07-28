use std::env;
use std::collections::VecDeque;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::exit;

fn depth_first_traverse(prog: &str, root: &Path, allow_dot: bool, follow_links: bool, iter_depth: i32, depth: i32) -> bool {
    let mut has_next_level = false;
    let entries = fs::read_dir(root);
    if let Ok(entries) = entries {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();

                let is_link = path.read_link().is_ok();
                let mut is_dot = false;

                let file_name = path.file_name();
                if let Some(file_name) = file_name {
                    let file_name_bytes = file_name.as_bytes();
                    if !file_name_bytes.is_empty() && file_name_bytes[0] == b'.' {
                        is_dot = true;
                    }
                } else {
                    eprintln!("{}: {}: cannot get filename", prog, path.display());
                    continue;
                }

                if (follow_links || !is_link) && (allow_dot || !is_dot) {
                    if depth < iter_depth {
                        if path.is_dir() {
                            if depth_first_traverse(prog, &entry.path(), allow_dot, follow_links, iter_depth, depth + 1) {
                                has_next_level = true;
                            }
                        }
                    } else {
                        if path.is_dir() {
                            has_next_level = true;
                        }
                        println!("{}", path.display());
                    }
                }
            } else if let Err(e) = entry {
                eprintln!("{}: {}: {}", prog, root.display(), e);
                continue;
            }
        }
    } else if let Err(e) = entries {
        eprintln!("{}: {}: {}", prog, root.display(), e);
    }
    return has_next_level;
}

fn iterative_deepening(prog: &str, mut roots: Vec<String>, allow_dot: bool, follow_links: bool, max_depth: i32) {
    if roots.is_empty() {
        for depth in 1..=max_depth {
            if !depth_first_traverse(prog, Path::new("."), allow_dot, follow_links, depth, 1) {
                break;
            }
        }
    } else {
        for depth in 1..=max_depth {
            let mut i = 0 as usize;
            while i < roots.len() {
                if !depth_first_traverse(prog, Path::new(unsafe { &roots.get_unchecked(i) }), allow_dot, follow_links, depth, 1) {
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
    println!("{}: [-H] [-L] [-d DEPTH] [DIR ...] [VERB ...] [-- EXPR ...]", prog);
    exit(0);
}

fn main() {
    let mut args: VecDeque<String> = env::args().collect();

    let mut allow_dot = false;
    let mut follow_links = false;
    let mut roots: Vec<String> = vec![];
    let mut max_depth = i32::MAX;
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

    iterative_deepening(prog, roots, allow_dot, follow_links, max_depth);
}
