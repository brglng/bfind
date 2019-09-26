use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::exit;

mod path_queue;
use path_queue::PathQueue;

fn walk_dir(prog: &str, root: &str, allow_dot: bool, follow_links: bool) {
    let mut q = PathQueue::new().unwrap();

    q.push(PathBuf::from(root)).unwrap();

    while let Some(file) = q.pop().unwrap() {
        let entries = fs::read_dir(&file);
        if let Ok(entries) = entries {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();

                    let is_link = path.read_link().is_ok();
                    let mut is_dot = false;

                    let file_name = path.file_name();
                    if let Some(file_name) = file_name {
                        let file_name_str = file_name.to_str();
                        if let Some(file_name_str) = file_name_str {
                            if file_name_str.starts_with(".") {
                                is_dot = true;
                            }
                        } else {
                            eprintln!("{}: {}: bad filename", prog, path.display());
                            continue;
                        }
                    } else {
                        eprintln!("{}: {}: cannot get filename", prog, path.display());
                        continue;
                    }

                    if follow_links || !is_link {
                        if allow_dot || !is_dot {
                            println!("{}", path.display());
                            if path.is_dir() {
                                q.push(path).unwrap();
                            }
                        }
                    } else {
                        eprintln!("{}: {}: is a link", prog, path.display());
                    }
                } else if let Err(e) = entry {
                    eprintln!("{}: {}: {}", prog, file.display(), e);
                };
            }
        } else if let Err(e) = entries {
            eprintln!("{}: {}: {}", prog, file.display(), e);
        }
    }
}

fn print_help(prog: &str) {
    println!("{}: [-H] [-L] DIR", prog);
    exit(0);
}

fn main() {
    let mut args: Vec<String> = env::args().collect();

    let mut allow_dot = false;
    let mut follow_links = false;
    let mut root = String::from(".");

    let prog_path = args.remove(0);
    let prog = prog_path.rsplit('/').nth_back(0).unwrap();

    for arg in args {
        if arg == "-H" {
            allow_dot = true;
        } else if arg == "-L" {
            follow_links = true;
        } else if arg == "-h" || arg == "--help" {
            print_help(&prog);
        } else if arg.len() > 1 && arg.starts_with("-") {
            eprintln!("{}: unknown argument: {}", &prog, &arg);
            exit(-1);
        } else {
            root = arg.clone();
        }
    }

    walk_dir(prog, &root, allow_dot, follow_links);
}
