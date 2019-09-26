use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use std::time::Duration;

fn worker(sender: Sender<PathBuf>, receiver: Receiver<PathBuf>) {
    while let Ok(file) = receiver.recv_timeout(Duration::from_millis(1000)) {
        let entries = fs::read_dir(&file);
        if let Ok(entries) = entries {
            println!("{}", file.display());
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    println!("{}", path.display());
                    if path.is_dir() {
                        sender.send(path).unwrap();
                    }
                } else if let Err(e) = entry {
                    eprintln!("bfind: {}: {}", file.display(), e);
                };
            }
        } else if let Err(e) = entries {
            eprintln!("bfind: {}: {}", file.display(), e);
        };
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let root = if args.len() > 1 {
        &args[1]
    } else {
        "."
    };

    let (sender1, receiver1) = channel();
    let (sender2, receiver2) = channel();

    sender1.send(PathBuf::from(root)).unwrap();

    let t1 = thread::spawn(move|| {
        worker(sender1, receiver2);
    });

    let t2 = thread::spawn(move|| {
        worker(sender2, receiver1);
    });

    t1.join().unwrap();
    t2.join().unwrap();
}
