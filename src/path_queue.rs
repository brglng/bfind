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
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
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

    #[error("path_queue::SpinLockFailed")]
    SpinLockFailed,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Eq, PartialEq)]
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

    // safe if and only if there is only one push thread
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

    // safe if and only if there is only one pop thread
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

    // safe if and only if there is only one push thread
    pub fn push(&mut self, path: &Path) -> Result<()> {
        let writer = unsafe { &mut *self.writer.get() };
        writer.write_all(path.as_os_str().as_bytes())?;
        writer.write_all(b"\0")?;
        writer.flush()?;
        self.push_count.fetch_add(1, Ordering::Release);
        Ok(())
    }

    // safe if and only if there is only one pop thread
    pub fn pop(&mut self) -> Result<Option<PathBuf>> {
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
struct SpinLock {
    locked: AtomicBool
}

impl SpinLock {
    pub fn new() -> Self {
        SpinLock { locked: AtomicBool::new(false) }
    }

    pub fn try_lock(&self) -> Result<SpinLockGuard> {
        if let Ok(_) = self.locked.compare_exchange_weak(false, true, Ordering::Release, Ordering::Acquire) {
            Ok(SpinLockGuard { lock: self })
        } else {
            Err(Error::SpinLockFailed)
        }
    }

    pub fn unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

struct SpinLockGuard<'a> {
    pub lock: &'a SpinLock
}

impl<'a> Drop for SpinLockGuard<'a> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}

#[derive(Debug)]
pub struct PathQueue {
    push_count:     AtomicUsize,
    pop_count:      AtomicUsize,
    pushing:        SpinLock,
    popping:        SpinLock,
    spilling:       SpinLock,
    left:           UnsafeCell<MemPathQueue>,
    mid:            UnsafeCell<Option<TempfilePathQueue>>,
    right:          UnsafeCell<MemPathQueue>,
}

impl PathQueue {
    pub fn new(read_buf_len: u32, write_buf_len: u32) -> Result<Self> {
        assert!(read_buf_len > 0 && write_buf_len > 0);
        Ok(PathQueue {
            push_count: AtomicUsize::new(0),
            pop_count: AtomicUsize::new(0),
            pushing: SpinLock::new(),
            popping: SpinLock::new(),
            spilling: SpinLock::new(),
            left: UnsafeCell::new(MemPathQueue::new(read_buf_len)),
            mid: UnsafeCell::new(None),
            right: UnsafeCell::new(MemPathQueue::new(write_buf_len)),
        })
    }

    pub fn push(&self, path: PathBuf) -> Result<Option<PathBuf>> {
        if let Ok(_pushing) = self.pushing.try_lock() {
            let left = unsafe { &mut *self.left.get() };
            let mid = unsafe { &mut *self.mid.get() };
            let right = unsafe { &mut *self.right.get() };

            if let Some(path) = right.push(path) {
                if let Ok(_spilling) = self.spilling.try_lock() {
                    if mid.is_none() {
                        *mid = Some(TempfilePathQueue::new()?);
                    }
                    let mid = unsafe { mid.as_mut().unwrap_unchecked() };
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
                } else {
                    return Ok(Some(path));
                }
            }

            self.push_count.fetch_add(1, Ordering::Release);

            Ok(None)
        } else {
            Ok(Some(path))
        }
    }

    pub fn pop(&self) -> Result<Option<PathBuf>> {
        if let Ok(_popping) = self.popping.try_lock() {
            let left = unsafe { &mut *self.left.get() };
            let mid = unsafe { &mut *self.mid.get() };
            let right = unsafe { &mut *self.right.get() };

            if Wrapping(self.push_count.load(Ordering::Acquire)) - Wrapping(self.pop_count.load(Ordering::Acquire)) == Wrapping(0) {
                return Ok(None)
            }

            let path;
            if let Ok(_spilling) = self.spilling.try_lock() {
                if let Some(p) = left.pop() {
                    path = p;
                } else if let Some(mid) = mid {
                    if let Some(p) = mid.pop()? {
                        path = p;
                    } else if let Some(p) = right.pop() {
                        path = p;
                    } else {
                        return Ok(None);
                    }
                } else if let Some(p) = right.pop() {
                    path = p;
                } else {
                    return Ok(None);
                }
            } else {
                return Ok(None);
            }

            self.pop_count.fetch_add(1, Ordering::Release);

            Ok(Some(path))
        } else {
            Ok(None)
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        Wrapping(self.push_count.load(Ordering::Acquire)) - Wrapping(self.pop_count.load(Ordering::Acquire)) == Wrapping(0)
    }

    #[cfg(test)]
    pub fn state(&self) -> (PathQueueState, PathQueueState, PathQueueState) {
        loop {
            if let Ok(_pushing) = self.pushing.try_lock() {
                if let Ok(_popping) = self.popping.try_lock() {
                    let left = unsafe { &*self.left.get() };
                    let mid = unsafe { &*self.mid.get() };
                    let right = unsafe { &*self.right.get() };
                    if let Some(mid) = mid {
                        return (left.state(), mid.state(), right.state())
                    } else {
                        return (left.state(), PathQueueState::Empty, right.state())
                    }
                }
            }
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
        assert_eq!(q.pop()?, Some(PathBuf::from("1")));
        assert_eq!(q.pop()?, Some(PathBuf::from("2")));
        assert_eq!(q.pop()?, Some(PathBuf::from("3")));
        assert_eq!(q.pop()?, Some(PathBuf::from("4")));
        assert_eq!(q.pop()?, Some(PathBuf::from("5")));
        assert_eq!(q.pop()?, Some(PathBuf::from("6")));
        Ok(())
    }

    #[test]
    fn spsc() -> Result<()> {
        let queue = PathQueue::new(73, 131)?;
        let count = 100000;
        thread::scope(|s| -> Result<()> {
            s.spawn(|| -> Result<()> {
                let mut i = 0;
                loop {
                    if let Some(path) = queue.pop()? {
                        eprintln!("popped {}", path.display());
                        assert_eq!(path.to_str().unwrap(), i.to_string());
                        i += 1;
                        if i == count {
                            break;
                        }
                    }
                }
                Ok(())
            });
            s.spawn(|| -> Result<()> {
                for i in 0..count {
                    let mut path = PathBuf::from(i.to_string());
                    let path_string = path.to_str().unwrap().to_string();
                    while let Some(p) = queue.push(path)? {
                        path = p;
                    }
                    eprintln!("pushed {}", path_string);
                }
                Ok(())
            });
            Ok(())
        })?;
        Ok(())
    }
}
