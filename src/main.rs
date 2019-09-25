use std::env;
use std::fs;
use std::fs::DirEntry;
use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, SyncSender, Receiver};
use std::thread;
use std::time::Duration;

fn worker(sender: SyncSender<PathBuf>, receiver: Receiver<PathBuf>) {
    while let Ok(file) = receiver.recv_timeout(Duration::from_secs(2)) {
        println!("{}", &file.display());
        if file.as_os_str().len() == 0 {
            break;
        }

        if file.is_dir() {
            let _ = fs::read_dir(&file).map(|entries| {
                for entry in entries {
                    let _ = entry.map(|entry: DirEntry| {
                        println!("{}", &entry.path().display());
                        if entry.path().is_dir() {
                            sender.send(entry.path()).unwrap();
                        }
                    }).map_err(|err| {
                        eprintln!("bfind: {}: {}", &file.display(), err);
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

    let root = if args.len() > 1 {
        &args[1]
    } else {
        "."
    };

    let (sender1, receiver1) = sync_channel(8192);
    let (sender2, receiver2) = sync_channel(8192);

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
