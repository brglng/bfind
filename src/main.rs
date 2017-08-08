use std::collections::VecDeque;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::io::Write;

macro_rules! println_stderr(
    ($($arg:tt)*) => { {
        match writeln!(&mut ::std::io::stderr(), $($arg)*) {
            Ok(_) => {},
            Err(_) => {}
        }
    } }
);

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut q = VecDeque::new();
    if args.len() > 1 {
        q.push_back(PathBuf::from(&args[1]));
    } else {
        q.push_back(PathBuf::from(r"."));
    }

    loop {
        let file = q.pop_front();
        match file {
            None => break,
            Some(file) => {
                println!("{}", &file.display());
                if file.is_dir() {
                    fs::read_dir(&file).map(|entries| {
                        for entry in entries {
                            entry.map(|entry| {
                                q.push_back(entry.path());
                            }).map_err(|err| {
                                println_stderr!("bfind: {}", err.to_string());
                            });
                        }
                    })
                    .map_err(|err| {
                        println_stderr!("bfind: {}: {}", file.display(), err.to_string());
                    });
                }
            }
        }
    }
}
