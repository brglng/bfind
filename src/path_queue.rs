use std::ffi::OsStr;
use std::io;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::fs::File;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::mpsc::{Sender, Receiver};
use tempfile::NamedTempFile;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("std::io::Error {{ kind = {} }}: {source}", source.kind())]
    Io {
        #[from]
        source: io::Error
    },

    #[error("mpsc::RecvError: {source}")]
    Recv {
        #[from]
        source: mpsc::RecvError
    },

    #[error("mpsc::SendError<PathBuf>: {source}")]
    Send {
        #[from]
        source: mpsc::SendError<PathBuf>
    },
}

pub type Result<T> = std::result::Result<T, Error>;

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
        self.writer.write_all(path.as_os_str().as_bytes())?;
        self.writer.write_all(b"\0")?;
        self.len += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Result<PathBuf> {
        let mut buffer = vec![];
        let num_bytes = self.reader.read_until(b'\0', &mut buffer)?;
        if num_bytes == 0 {
            self.writer.flush()?;
            self.reader.read_until(b'\0', &mut buffer)?;
        }
        buffer.pop();
        self.len -= 1;
        Ok(PathBuf::from(OsStr::from_bytes(&buffer)))
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
    q:                          Storage,
    mem_to_tempfile_thresh:     usize,
    tempfile_to_mem_thresh:     usize,
}

impl PathQueue {
    pub fn new(mem_to_tempfile_thresh: usize, tempfile_to_mem_thresh: usize) -> Self {
        PathQueue{ q: Storage::Mem(MemPathQueue::new()), mem_to_tempfile_thresh, tempfile_to_mem_thresh }
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
                if memq.len() < self.mem_to_tempfile_thresh {
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
                if tempfileq.len() < self.tempfile_to_mem_thresh {
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
        let mut q = PathQueue::new(4, 2);
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
