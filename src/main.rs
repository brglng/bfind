use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::PathBuf;

fn walk_dir(root_dir: String) {
    let mut q = VecDeque::new();

    q.push_back(PathBuf::from(root_dir));

    while let Some(file) = q.pop_front() {
        if file.is_dir() {
            let _ = fs::read_dir(&file).map(|entries| {
                for entry in entries {
                    let _ = entry.map(|entry| {
                        println!("{}", &entry.path().display());
                        if entry.path().is_dir() {
                            q.push_back(entry.path());
                        }
                    }).map_err(|err| {
                        eprintln!("bfind: {}", err);
                    });
                }
            }).map_err(|err| {
                eprintln!("bfind: {}: {}", &file.display(), err);
            });
        } else {
            // There are some cases in which entry.path().is_dir() above
            // returned true but here file.is_dir() returned false. This often
            // occurs when the file is a symlink to a directory but we don't
            // have access to it. Don't know why though. Just ignore it.
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let root_dir = if args.len() > 1 {
        args[1].clone()
    } else {
        String::from(".")
    };

    walk_dir(root_dir);
}
