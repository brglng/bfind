use std::env;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::exit;

fn depth_first_traverse(prog: &str, root: &Path, allow_dot: bool, follow_links: bool, iter_depth: i32, depth: i32) {
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
                            depth_first_traverse(prog, &entry.path(), allow_dot, follow_links, iter_depth, depth + 1);
                        }
                    } else {
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
}

fn iterative_deepening(prog: &str, root: &Path, allow_dot: bool, follow_links: bool, max_depth: i32) {
    for depth in 1..=max_depth {
        depth_first_traverse(prog, root, allow_dot, follow_links, depth, 1);
    }
}

fn print_help(prog: &str) {
    println!("{}: [-H] [-L] [starting-point...] [expression]", prog);
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
            print_help(prog);
        } else if arg.len() > 1 && arg.starts_with('-') {
            eprintln!("{}: unknown argument: {}", &prog, &arg);
            exit(-1);
        } else {
            root.clone_from(&arg);
        }
    }

    iterative_deepening(prog, Path::new(&root), allow_dot, follow_links, 100);
}
