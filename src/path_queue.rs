#![allow(dead_code)]

use std::error;
use std::fmt;
use std::io;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::fs::File;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::mpsc::{Sender, Receiver};
extern crate tempfile;
use self::tempfile::NamedTempFile;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    RecvError(mpsc::RecvError),
    SendError(mpsc::SendError<PathBuf>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::IoError(ref e) => e.fmt(f),
            Self::RecvError(ref e) => e.fmt(f),
            Self::SendError(ref e) => e.fmt(f),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            Self::IoError(ref e) => Some(e),
            Self::RecvError(ref e) => Some(e),
            Self::SendError(ref e) => Some(e),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::IoError(err)
    }
}

impl From<mpsc::RecvError> for Error {
    fn from(err: mpsc::RecvError) -> Self {
        Self::RecvError(err)
    }
}

impl From<mpsc::SendError<PathBuf>> for Error {
    fn from(err: mpsc::SendError<PathBuf>) -> Self {
        Self::SendError(err)
    }
}

struct TempfilePathQueue {
    writer:     BufWriter<File>,
    reader:     BufReader<File>,
    len:        usize,
}

impl TempfilePathQueue {
    pub fn new() -> Result<Self> {
        let f = NamedTempFile::new()?;
        let writer = BufWriter::with_capacity(1024 * 1024 * 16, f.reopen()?);
        let reader = BufReader::with_capacity(1024 * 1024 * 16, f.reopen()?);
        drop(f);
        Ok(TempfilePathQueue{writer, reader, len: 0})
    }

    pub fn len(&self) -> usize { self.len }

    pub fn push(&mut self, path: PathBuf) -> Result<()> {
        writeln!(self.writer, "{}", &path.display())?;
        self.len += 1;
        return Ok(());
    }

    pub fn pop(&mut self) -> Result<PathBuf> {
        let mut buffer = String::new();
        let len = self.reader.read_line(&mut buffer)?;
        if len == 0 {
            self.writer.flush()?;
            self.reader.read_line(&mut buffer)?;
        }
        self.len -= 1;
        Ok(PathBuf::from(buffer.trim_end()))
    }
}

struct MemPathQueue {
    tx:     Sender<PathBuf>,
    rx:     Receiver<PathBuf>,
    len:    usize
}

impl MemPathQueue {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        MemPathQueue{tx, rx, len: 0}
    }

    pub fn len(&self) -> usize { self.len }

    pub fn push(&mut self, path: PathBuf) -> Result<()> {
        self.tx.send(path)?;
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Result<PathBuf> {
        let path = self.rx.recv()?;
        self.len -= 1;
        Ok(path)
    }
}

enum Storage {
    Mem(MemPathQueue),
    Tempfile(TempfilePathQueue),
}

// This queue stores the queue to a disk file if the queue is too large.
pub struct PathQueue {
    q:              Storage,
    max_mem_len:    usize,
}

impl PathQueue {
    pub fn new(max_mem_len: usize) -> Self {
        PathQueue{ q: Storage::Mem(MemPathQueue::new()), max_mem_len }
    }

    pub fn len(&self) -> usize {
        match self.q {
            Storage::Mem(ref q) => q.len(),
            Storage::Tempfile(ref q) => q.len(),
        }
    }

    #[cfg(test)]
    pub fn is_mem(&self) -> bool {
        match self.q {
            Storage::Mem(ref _q) => true,
            Storage::Tempfile(ref _q) => false,
        }
    }

    #[cfg(test)]
    pub fn is_tempfile(&self) -> bool {
        match self.q {
            Storage::Mem(ref _q) => false,
            Storage::Tempfile(ref _q) => true,
        }
    }

    pub fn push(&mut self, path: PathBuf) -> Result<()> {
        match self.q {
            Storage::Mem(ref mut memq) => {
                if memq.len() < self.max_mem_len {
                    return memq.push(path);
                } else {
                    let mut tempfileq = TempfilePathQueue::new()?;
                    for _ in 0..memq.len() {
                        tempfileq.push(memq.pop()?)?;
                    }
                    tempfileq.push(path)?;
                    self.q = Storage::Tempfile(tempfileq);
                    return Ok(());
                }
            },
            Storage::Tempfile(ref mut q) => {
                return q.push(path);
            }
        }
    }

    pub fn pop(&mut self) -> Result<PathBuf> {
        match self.q {
            Storage::Mem(ref mut memq) => {
                memq.pop()
            },
            Storage::Tempfile(ref mut tempfileq) => {
                let path = tempfileq.pop()?;
                if tempfileq.len() < self.max_mem_len / 2 {
                    let mut memq = MemPathQueue::new();
                    for _ in 0..tempfileq.len() {
                        memq.push(tempfileq.pop()?)?;
                    }
                    self.q = Storage::Mem(memq);
                }
                return Ok(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn path_queue() -> Result<()> {
        let mut q = PathQueue::new(4);
        q.push(PathBuf::from("a/b"))?;
        q.push(PathBuf::from("b/c"))?;
        q.push(PathBuf::from("c/d"))?;
        q.push(PathBuf::from("d/e"))?;
        assert_eq!(q.len(), 4);
        assert!(q.is_mem());
        q.push(PathBuf::from("e/f"))?;
        assert_eq!(q.len(), 5);
        assert!(q.is_tempfile());
        assert_eq!(q.pop()?, PathBuf::from("a/b"));
        assert_eq!(q.pop()?, PathBuf::from("b/c"));
        assert_eq!(q.pop()?, PathBuf::from("c/d"));
        assert!(q.is_tempfile());
        assert_eq!(q.pop()?, PathBuf::from("d/e"));
        assert!(q.is_mem());
        assert_eq!(q.pop()?, PathBuf::from("e/f"));
        assert_eq!(q.len(), 0);
        Ok(())
    }
}
