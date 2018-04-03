use std::io;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::fs;
use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::env;
extern crate rand;
use self::rand::Rng;

// This queue stores the queue to a disk file if the queue is too large.
pub struct PathQueue {
    writer:     BufWriter<File>,
    reader:     BufReader<File>,
    len:        usize,
    filepath:   PathBuf
}

impl PathQueue {
    pub fn new() -> Result<PathQueue, io::Error> {
        let mut rng = rand::thread_rng();
        let mut tmpfilename: String = "bfind.tmp.".to_owned();
        tmpfilename.push_str(&rng.gen::<u32>().to_string());
        let full_tmpfilename = env::temp_dir().join(&tmpfilename);

        // eprintln!("using temp file: {}", &full_tmpfilename.display());

        let fwrite = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&full_tmpfilename)?;

        let writer = BufWriter::new(fwrite);

        let fread = OpenOptions::new().read(true).open(&full_tmpfilename)?;
        let reader = BufReader::new(fread);

        Ok(PathQueue{writer: writer, reader: reader, len: 0, filepath: full_tmpfilename})
    }

    pub fn push(&mut self, path: PathBuf) -> Result<(), io::Error> {
        let mut result = writeln!(self.writer, "{}", &path.display());
        match result {
            Ok(()) => {
                self.len += 1;
            },
            Err(e) => {
                result = Err(e);
            }
        }
        return result;
    }

    pub fn pop(&mut self) -> Result<Option<PathBuf>, io::Error> {
        if self.len > 0 {
            let mut buffer = String::new();
            let len = self.reader.read_line(&mut buffer)?;
            // println!("{}", len);
            if len == 0 {
                self.writer.flush()?;
                self.reader.read_line(&mut buffer)?;
            }
            self.len -= 1;
            Ok(Some(PathBuf::from(buffer.trim_right())))
        } else {
            Ok(None)
        }
    }

    pub fn remove_temp_file(&mut self) -> Result<(), io::Error> {
        fs::remove_file(&self.filepath).map_err(|err| {
            eprintln!("failed to remove file: {}: {}", &self.filepath.display(), err);
            err
        })
    }
}

impl Drop for PathQueue {
    fn drop(&mut self) {
        let _ = self.remove_temp_file();
    }
}

