use std::io;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use std::fs::File;
use std::path::PathBuf;
extern crate tempfile;
use self::tempfile::NamedTempFile;

// This queue stores the queue to a disk file if the queue is too large.
pub struct PathQueue {
    writer:     BufWriter<File>,
    reader:     BufReader<File>,
    len:        usize,
}

impl PathQueue {
    pub fn new() -> Result<Self, io::Error> {
        let f = NamedTempFile::new()?;
        let writer = BufWriter::new(f.reopen()?);
        let reader = BufReader::new(f.reopen()?);
        drop(f);
        Ok(PathQueue{writer: writer, reader: reader, len: 0})
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
}
