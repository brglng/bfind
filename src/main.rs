use std::collections::VecDeque;
use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::exit;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
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
    },
}

type Result<T> = std::result::Result<T, Error>;

struct Options {
    allow_hidden:       bool,
    follow_links:       bool,
    max_depth:          i32,
    ignores:            Vec<String>,
    strip_cwd_prefix:   bool,
}

impl Options {
    pub fn new() -> Self {
        Self {
            allow_hidden: false,
            follow_links: false,
            max_depth: i32::MAX,
            ignores: Vec::new(),
            strip_cwd_prefix: false,
        }
    }
}

fn pop_or_steal(queues: &[PathQueue], index: usize) -> Result<Option<PathBuf>> {
    if let Some(path) = queues[index].pop()? {
        Ok(Some(path))
    } else {
        for (i, queue) in queues.iter().enumerate() {
            if i != index {
                if let Some(path) = queue.pop()? {
                    return Ok(Some(path));
                }
            }
        }
        Ok(None)
    }
}

fn push(queues: &[PathQueue], index: usize, path: PathBuf) -> Result<()> {
    if let Some(mut path) = queues[index].push(path)? {
        loop {
            for (i, queue) in queues.iter().enumerate() {
                if i != index {
                    if let Some(p) = queue.push(path)? {
                        path = p;
                    } else {
                        return Ok(());
                    }
                }
            }
            if let Some(p) = queues[index].push(path)? {
                path = p;
            } else {
                return Ok(());
            }
        }
    }
    Ok(())
}

fn breadth_first_traverse(prog: &str, cwd: &Path, opt: &Options, queues: &[PathQueue], index: usize, counter: &AtomicUsize) -> Result<()> {
    loop {
        let path = pop_or_steal(queues, index)?;
        if let Some(path) = path {
            let entries = fs::read_dir(&path);
            if let Ok(entries) = entries {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let mut path = entry.path();
                        if opt.follow_links && path.is_symlink() {
                            let p = path.read_link();
                            if let Ok(p) = p {
                                path = p;
                            } else {
                                eprintln!("{}: {}: {}", prog, path.display(), p.unwrap_err());
                                continue;
                            }
                        }
                        if let Some(file_name) = entry.file_name().to_str() {
                            if !opt.allow_hidden {
                                if let Some(first_char) = file_name.chars().next() {
                                    if first_char == '.' {
                                        continue;
                                    }
                                }
                            }
                            if opt.ignores.iter().any(|item| item == file_name) {
                                continue;
                            }
                            if opt.strip_cwd_prefix {
                                if path.starts_with("./") {
                                    println!("{}", unsafe { path.strip_prefix("./").unwrap_unchecked() }.display());
                                } else if path.starts_with(cwd) {
                                    println!("{}", unsafe { path.strip_prefix(cwd).unwrap_unchecked() }.display());
                                } else {
                                    println!("{}", path.display());
                                }
                            } else {
                                println!("{}", path.display());
                            }
                            if path.is_dir() {
                                push(queues, index, path)?;
                                counter.fetch_add(1, Ordering::Release);
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
            counter.fetch_sub(1, Ordering::Release);
        } else if counter.load(Ordering::Acquire) == 0 {
            break;
        } else {
            thread::sleep(Duration::from_millis(7));
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

    let mut state = CliState::Options;
    let mut roots: Vec<String> = Vec::new();
    let mut opts = Options::new();
    let mut verb = Verb::Print;
    let mut action_tokens = Vec::new();
    let mut expr_tokens: Vec<String> = Vec::new();

    let prog_path = args.pop_front().unwrap();
    let prog = prog_path.rsplit('/').nth_back(0).unwrap();
    let cwd = env::current_dir().unwrap_or_else(|e| {
        eprintln!("{}: {}", prog, e);
        exit(1);
    });

    while let Some(arg) = args.pop_front() {
        match state {
            CliState::Options => {
                if arg == "-H" {
                    opts.allow_hidden = true;
                } else if arg == "-L" {
                    opts.follow_links = true;
                } else if arg == "-h" || arg == "--help" {
                    print_help(prog);
                } else if arg == "-d" || arg == "--depth" {
                    if let Some(depth_str) = args.pop_front() {
                        if let Ok(depth) = depth_str.parse::<i32>() {
                            if depth < 1 {
                                eprintln!("{}: depth must be > 0", prog);
                                exit(1);
                            }
                            opts.max_depth = depth;
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
                        opts.ignores = ignore.split(',').map(|s| { s.to_string() }).collect();
                    } else {
                        eprintln!("{}: missing argument to -I", prog);
                        exit(1);
                    }
                } else if arg == "--strip-cwd-prefix" {
                    opts.strip_cwd_prefix = true;
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
            n.get() + 1
        } else {
            exit(1);
        }
    };

    let mut queues = Vec::new();
    for _ in 0..num_threads {
        let q = PathQueue::new((1024 * 512 / num_threads) as u32, (1024 * 512 / num_threads) as u32);
        if let Ok(q) = q {
            queues.push(q);
        } else {
            eprintln!("{}: {}", prog, q.unwrap_err());
            exit(1);
        }
    }

    let mut counter: usize = 0;

    if roots.is_empty() {
        if let Err(e) = queues[0].push(PathBuf::from(".")) {
            eprintln!("{}: {}", prog, e);
            exit(1);
        }
        counter = 1;
    } else {
        let dotdir = Path::new(".");
        let dotdotdir = Path::new("..");
        let rootdir = Path::new("/");
        for root in &roots {
            let path = PathBuf::from(root);
            if !opts.follow_links && path.is_symlink() {
                continue;
            }
            if path != dotdir && path != dotdotdir && path != rootdir {
                eprintln!("{}", path.display());
                if let Some(file_name) = path.file_name() {
                    if let Some(file_name) = file_name.to_str() {
                        if !opts.allow_hidden {
                            if let Some(first_char) = file_name.chars().next() {
                                if first_char == '.' {
                                    continue;
                                }
                            }
                        }
                        if opts.ignores.iter().any(|item| item == file_name) {
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
            } else {
                counter += 1;
            }
        }
    }

    let counter = AtomicUsize::new(counter);
    if let Err(e) = thread::scope(|s| -> Result<()> {
        let cwd = &cwd;
        let opts = &opts;
        let queues = &queues;
        let counter = &counter;
        for i in 0..num_threads {
            s.spawn(move|| -> Result<()> {
                breadth_first_traverse(prog, cwd, opts, queues, i, counter)
            });
        }
        Ok(())
    }) {
        eprintln!("{}: {}", prog, e);
    }
}
