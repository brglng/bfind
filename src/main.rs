use std::env;
use std::fs;
use std::path::PathBuf;

mod path_queue;
use path_queue::PathQueue;

fn walk_dir(root: String) {
    let mut q = PathQueue::new().unwrap();

    q.push(PathBuf::from(root)).unwrap();

    while let Some(file) = q.pop().unwrap() {
        if file.as_os_str().len() == 0 {
            break;
        }

        if file.is_dir() {
            let _ = fs::read_dir(&file).map(|entries| {
                for entry in entries {
                    let _ = entry.map(|entry| {
                        println!("{}", &entry.path().display());
                        if entry.path().is_dir() {
                            q.push(entry.path()).unwrap();
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

#[allow(dead_code)]
fn dls(root: PathBuf, depth: usize) -> usize {
    let mut num_all_children: usize = 0;

    if depth == 0 {
        num_all_children += 1;
        println!("{}", &root.display());
    } else {
        if root.is_dir() {
            let _ = fs::read_dir(&root).map(|entries| {
                for entry in entries {
                    let _ = entry.map(|entry| {
                        num_all_children += dls(entry.path(), depth - 1);
                    }).map_err(|err| {
                        eprintln!("bfind: {}", &err);
                    });
                }
            }).map_err(|err| {
                eprintln!("bfind: {}: {}", &root.display(), &err);
            });
        }
    }

    return num_all_children;
}

#[allow(dead_code)]
fn iddfs(root: String, min_depth: usize, max_depth: usize) {
    for i in min_depth..max_depth {
        if dls(PathBuf::from(&root), i) == 0 {
            break;
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let root = if args.len() > 1 {
        args[1].clone()
    } else {
        String::from(".")
    };

    walk_dir(root);
    // iddfs(root, 0, 50);
}
