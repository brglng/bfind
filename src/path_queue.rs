use std::alloc::alloc;
use std::alloc::dealloc;
use std::alloc::Layout;
use std::cell::UnsafeCell;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Write;
use std::mem::align_of;
use std::mem::size_of;
use std::num::Wrapping;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Condvar;
use std::time::Duration;
use std::sync::Mutex;
use tempfile::NamedTempFile;
use thiserror::Error;

#[allow(unused_imports)]
use debug_print::{debug_println, debug_eprintln};

#[derive(Error, Debug)]
pub enum Error {
    #[error("std::io::Error {{ kind = {} }}: {source}", source.kind())]
    Io {
        #[from]
        source: io::Error
    },
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, PartialEq)]
pub enum PathQueueState {
    Empty,
    PartiallyFilled,

    #[allow(dead_code)]
    Full,
}

#[allow(dead_code)]
impl PathQueueState {
    pub fn is_empty(&self) -> bool {
        matches!(self, PathQueueState::Empty)
    }

    pub fn is_partially_filled(&self) -> bool {
        matches!(self, PathQueueState::PartiallyFilled)
    }

    pub fn is_full(&self) -> bool {
        matches!(self, PathQueueState::Full)
    }
}

#[derive(Debug)]
struct MemPathQueue {
    capacity:       u32,
    pop_count:      AtomicU32,
    push_count:     AtomicU32,
    buf:            *mut PathBuf,
}

impl MemPathQueue {
    pub fn new(capacity: u32) -> Self {
        let capacity = capacity.next_power_of_two();
        let buf = unsafe {
            let layout = Layout::from_size_align(size_of::<PathBuf>() * capacity as usize, align_of::<PathBuf>()).expect("Bad layout");
            alloc(layout) as *mut PathBuf
        };
        Self {
            capacity,
            pop_count: AtomicU32::new(0),
            push_count: AtomicU32::new(0),
            buf
        }
    }

    pub fn push(&mut self, path: PathBuf) -> Option<PathBuf> {
        let push_count = self.push_count.load(Ordering::Acquire);
        let pop_count = self.pop_count.load(Ordering::Acquire);
        if push_count - pop_count == self.capacity {
            return Some(path);
        }
        unsafe {
            self.buf.add((push_count & (self.capacity - 1)) as usize).write(path);
        }
        self.push_count.fetch_add(1, Ordering::Release);
        None
    }

    pub fn pop(&mut self) -> Option<PathBuf> {
        let push_count = self.push_count.load(Ordering::Acquire);
        let pop_count = self.pop_count.load(Ordering::Acquire);
        if push_count - pop_count == 0 {
            return None;
        }
        let path = unsafe {
            self.buf.add((pop_count & (self.capacity - 1)) as usize).read()
        };
        self.pop_count.fetch_add(1, Ordering::Release);
        Some(path)
    }

    #[allow(dead_code)]
    pub fn state(&self) -> PathQueueState {
        let push_count = self.push_count.load(Ordering::Acquire);
        let pop_count = self.pop_count.load(Ordering::Acquire);
        if push_count - pop_count == 0 {
            PathQueueState::Empty
        } else if push_count - pop_count == self.capacity {
            PathQueueState::Full
        } else {
            PathQueueState::PartiallyFilled
        }
    }
}

impl Drop for MemPathQueue {
    fn drop(&mut self) {
        unsafe {
            let layout = Layout::from_size_align(size_of::<PathBuf>() * self.capacity as usize, align_of::<PathBuf>()).expect("Bad layout");
            dealloc(self.buf as *mut u8, layout);
        }
    }
}

unsafe impl Sync for MemPathQueue {}

#[derive(Debug)]
struct TempfilePathQueue {
    pop_count:      AtomicUsize,
    push_count:     AtomicUsize,
    writer:         UnsafeCell<BufWriter<File>>,
    reader:         UnsafeCell<BufReader<File>>,
}

impl TempfilePathQueue {
    pub fn new() -> Result<Self> {
        let f = NamedTempFile::new()?;
        let q = Self {
            pop_count: AtomicUsize::new(0),
            push_count: AtomicUsize::new(0),
            writer: UnsafeCell::new(BufWriter::new(f.reopen()?)),
            reader: UnsafeCell::new(BufReader::new(f.reopen()?)),
        };
        drop(f);
        Ok(q)
    }

    pub fn push(&self, path: &Path) -> Result<()> {
        let writer = unsafe { &mut *self.writer.get() };
        writer.write_all(path.as_os_str().as_bytes())?;
        writer.write_all(b"\0")?;
        writer.flush()?;
        self.push_count.fetch_add(1, Ordering::Release);
        Ok(())
    }

    pub fn pop(&self) -> Result<Option<PathBuf>> {
        let reader = unsafe { &mut *self.reader.get() };
        let push_count = self.push_count.load(Ordering::Acquire);
        let pop_count = self.pop_count.load(Ordering::Acquire);
        if push_count - pop_count == 0 {
            return Ok(None);
        }
        let mut buffer = vec![];
        reader.read_until(b'\0', &mut buffer)?;
        let delim = buffer.pop();
        assert_eq!(delim, Some(b'\0'));
        self.pop_count.fetch_add(1, Ordering::Release);
        let path = PathBuf::from(OsStr::from_bytes(&buffer));
        Ok(Some(path))
    }

    pub fn state(&self) -> PathQueueState {
        let push_count = self.push_count.load(Ordering::Acquire);
        let pop_count = self.pop_count.load(Ordering::Acquire);
        if push_count - pop_count == 0 {
            return PathQueueState::Empty;
        }
        PathQueueState::PartiallyFilled
    }
}

unsafe impl Sync for TempfilePathQueue {}

#[derive(Debug)]
pub struct PathQueue {
    push_count:     Mutex<Wrapping<usize>>,
    push_cond:      Condvar,
    pop_count:      AtomicUsize,
    push_mutex:     Mutex<()>,
    pop_mutex:      Mutex<()>,
    spill_mutex:    Mutex<()>,
    left:           UnsafeCell<MemPathQueue>,
    mid:            UnsafeCell<Option<TempfilePathQueue>>,
    right:          UnsafeCell<MemPathQueue>,
}

impl PathQueue {
    pub fn new(read_buf_len: u32, write_buf_len: u32) -> Result<Self> {
        assert!(read_buf_len > 0 && write_buf_len > 0);
        Ok(PathQueue {
            push_count: Mutex::new(Wrapping(0)),
            push_cond: Condvar::new(),
            pop_count: AtomicUsize::new(0),
            push_mutex: Mutex::new(()),
            pop_mutex: Mutex::new(()),
            spill_mutex: Mutex::new(()),
            left: UnsafeCell::new(MemPathQueue::new(read_buf_len)),
            mid: UnsafeCell::new(None),
            right: UnsafeCell::new(MemPathQueue::new(write_buf_len)),
        })
    }

    // only safe when there is only a single pusher
    pub fn push(&self, path: PathBuf) -> Result<()> {
        let _push_guard = self.push_mutex.lock().unwrap();

        let left = unsafe { &mut *self.left.get() };
        let mid = unsafe { &mut *self.mid.get() };
        let right = unsafe { &mut *self.right.get() };

        if let Some(path) = right.push(path) {
            let _spill_guard = self.spill_mutex.lock().unwrap();
            if mid.is_none() {
                *mid = Some(TempfilePathQueue::new()?);
            }
            let mid = unsafe { mid.as_ref().unwrap_unchecked() };
            if mid.state().is_empty() {
                while let Some(p) = right.pop() {
                    if let Some(p) = left.push(p) {
                        mid.push(&p)?;
                    }
                }
            } else {
                while let Some(p) = right.pop() {
                    mid.push(&p)?;
                }
            }
            right.push(path);
        }

        {
            let mut push_count = self.push_count.lock().unwrap();
            *push_count += 1;
            self.push_cond.notify_one();
        }

        Ok(())
    }

    // only safe when there is only a single popper
    pub fn pop_timeout(&self, ms: u64) -> Result<Option<PathBuf>> {
        let _pop_guard = self.pop_mutex.lock().unwrap();

        let left = unsafe { &mut *self.left.get() };
        let mid = unsafe { &*self.mid.get() };
        let right = unsafe { &mut *self.right.get() };

        if ms == 0 {
            let _push_count = self.push_cond.wait_while(
                self.push_count.lock().unwrap(), 
                |&mut push_count| push_count - Wrapping(self.pop_count.load(Ordering::Acquire)) == Wrapping(0)).unwrap();
        } else {
            let result = self.push_cond.wait_timeout_while(
                self.push_count.lock().unwrap(), 
                Duration::from_millis(ms), 
                |&mut push_count| push_count - Wrapping(self.pop_count.load(Ordering::Acquire)) == Wrapping(0)).unwrap();
            if result.1.timed_out() {
                return Ok(None);
            }
        }

        let path;
        loop {
            let _spill_guard = self.spill_mutex.lock().unwrap();
            if let Some(p) = left.pop() {
                path = p;
                break;
            } else if let Some(mid) = mid {
                if let Some(p) = mid.pop()? {
                    path = p;
                    break;
                } else if let Some(p) = right.pop() {
                    path = p;
                    break;
                }
            } else if let Some(p) = right.pop() {
                path = p;
                break;
            }
        }

        self.pop_count.fetch_add(1, Ordering::Release);

        Ok(Some(path))
    }

    #[allow(dead_code)]
    pub fn pop(&self) -> Result<PathBuf> {
        let path = self.pop_timeout(0)?;
        Ok(unsafe { path.unwrap_unchecked() })
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        let push_count = self.push_count.lock().unwrap();
        *push_count - Wrapping(self.pop_count.load(Ordering::Acquire)) == Wrapping(0)
    }

    #[cfg(test)]
    pub fn state(&self) -> (PathQueueState, PathQueueState, PathQueueState) {
        let _push_guard= self.push_mutex.lock().unwrap();
        let _pop_guard = self.pop_mutex.lock().unwrap();
        let left = unsafe { &*self.left.get() };
        let mid = unsafe { &*self.mid.get() };
        let right = unsafe { &*self.right.get() };
        if let Some(mid) = mid {
            (left.state(), mid.state(), right.state())
        } else {
            (left.state(), PathQueueState::Empty, right.state())
        }
    }
}

unsafe impl Sync for PathQueue {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn single_thread() -> Result<()> {
        let q = PathQueue::new(2, 2)?;
        q.push(PathBuf::from("1"))?;
        assert_eq!(q.state(), (PathQueueState::Empty, PathQueueState::Empty, PathQueueState::PartiallyFilled));
        q.push(PathBuf::from("2"))?;
        assert_eq!(q.state(), (PathQueueState::Empty, PathQueueState::Empty, PathQueueState::Full));
        q.push(PathBuf::from("3"))?;
        assert_eq!(q.state(), (PathQueueState::Full, PathQueueState::Empty, PathQueueState::PartiallyFilled));
        q.push(PathBuf::from("4"))?;
        assert_eq!(q.state(), (PathQueueState::Full, PathQueueState::Empty, PathQueueState::Full));
        q.push(PathBuf::from("5"))?;
        assert_eq!(q.state(), (PathQueueState::Full, PathQueueState::PartiallyFilled, PathQueueState::PartiallyFilled));
        q.push(PathBuf::from("6"))?;
        assert_eq!(q.state(), (PathQueueState::Full, PathQueueState::PartiallyFilled, PathQueueState::Full));
        assert_eq!(q.pop()?, PathBuf::from("1"));
        assert_eq!(q.pop()?, PathBuf::from("2"));
        assert_eq!(q.pop()?, PathBuf::from("3"));
        assert_eq!(q.pop()?, PathBuf::from("4"));
        assert_eq!(q.pop()?, PathBuf::from("5"));
        assert_eq!(q.pop()?, PathBuf::from("6"));
        Ok(())
    }

    #[test]
    fn spsc() -> Result<()> {
        let queue = PathQueue::new(73, 131)?;
        let count = 100000;
        thread::scope(|s| -> Result<()> {
            s.spawn(|| -> Result<()> {
                for i in 0..count {
                    let path = queue.pop()?;
                    eprintln!("popped {}", path.display());
                    assert_eq!(path.to_str().unwrap(), i.to_string());
                }
                Ok(())
            });
            s.spawn(|| -> Result<()> {
                for i in 0..count {
                    let path = PathBuf::from(i.to_string());
                    let path_string = path.to_str().unwrap().to_string();
                    queue.push(path)?;
                    eprintln!("pushed {}", path_string);
                }
                Ok(())
            });
            Ok(())
        })?;
        Ok(())
    }
}
