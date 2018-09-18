use std::collections::BTreeMap;
use syscall::error::{Error, Result, EBADF, EWOULDBLOCK};
use syscall::flag::O_NONBLOCK;
use syscall::scheme::SchemeBlockMut;

struct Handle {
    flags: usize,
    count: usize,
}

pub struct ExampleScheme {
    next_id: usize,
    count: usize,
    handles: BTreeMap<usize, Handle>,
}

impl ExampleScheme {
    pub fn new() -> Self {
        // Create a new scheme
        Self {
            next_id: 0,
            count: 0,
            handles: BTreeMap::new()
        }
    }

    pub fn irq(&mut self) -> bool {
        // Count number of IRQs
        self.count += 1;

        // This is a fake driver, so the device cannot have caused an IRQ
        false
    }
}

// Implementing `SchemeBlockMut` provides the `handle` function used in `main`
impl SchemeBlockMut for ExampleScheme {
    fn open(&mut self, _path: &[u8], flags: usize, _uid: u32, _gid: u32) -> Result<Option<usize>> {
        // `open` increments to the next id and stores the flags
        // Usually, `path`, `flags`, `uid`, and `gid` would be checked and used
        self.next_id += 1;
        let id = self.next_id;
        self.handles.insert(id, Handle {
            flags: flags,
            count: self.count
        });
        Ok(Some(id))
    }

    fn read(&mut self, id: usize, buf: &mut [u8]) -> Result<Option<usize>> {
        // `read` will succeed if the id exists
        let handle = self.handles.get_mut(&id).ok_or(Error::new(EBADF))?;

        // Put a '#' in the buffer for every interrupt
        let mut i = 0;
        while handle.count < self.count && i + 1 < buf.len() {
            buf[i] = b'#';
            buf[i + 1] = b'\n';
            handle.count += 1;
            i += 2;
        }

        if i > 0 {
            // Return count with `Some(i)`
            Ok(Some(i))
        } else if handle.flags & O_NONBLOCK == O_NONBLOCK {
            // Error with `EWOULDBLOCK`
            Err(Error::new(EWOULDBLOCK))
        } else {
            // Block with `None`
            Ok(None)
        }
    }

    fn fsync(&mut self, id: usize) -> Result<Option<usize>> {
        // `fsync` will always succeed if the id exists
        let _handle = self.handles.get(&id).ok_or(Error::new(EBADF))?;
        Ok(Some(0))
    }

    fn close(&mut self, id: usize) -> Result<Option<usize>> {
        // `close` removes an open id if it exists
        let _handle = self.handles.remove(&id).ok_or(Error::new(EBADF))?;
        Ok(Some(0))
    }
}
