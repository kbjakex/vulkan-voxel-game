/* use std::{sync::{atomic::{Ordering, AtomicU32}, Arc}, cell::UnsafeCell};

const RING_WRAP_MASK : usize = 64;

struct Shared {
    ring_buf: Vec<UnsafeCell<Option<Vec<u8>>>>,
    num_available: AtomicU32,
}

struct Reader {
    shared: Arc<Shared>,
    reader_idx: usize,
}

impl Reader {
    pub fn bulk_read(&mut self, out: &mut [Vec<u8>]) -> usize {
        let mut shared = &*self.shared;
        
        let mut available = usize::min(shared.num_available.load(Ordering::SeqCst) as usize, out.len());
        let mut num_read = 0usize;

        while available > 0 {
            for i in 0..available {
                let idx = (self.reader_idx + i) & RING_WRAP_MASK;
                out[num_read + i] = unsafe { &mut *shared.ring_buf[idx].get() }.take().unwrap();
            }
            self.reader_idx += available;
            num_read += available;
            available = usize::min(shared.num_available.fetch_sub(available as u32, Ordering::SeqCst) as usize - available, out.len() - num_read);
        }

        num_read
    }
}

struct Writer {
    shared: Arc<Shared>,
    writer_idx: usize,
}

impl Writer {
    pub fn bulk_write(&mut self, data: &[Vec<u8>]) -> usize {
        let shared = &*self.shared;  
    
        let mut writable = usize::min(RING_WRAP_MASK - shared.num_available.load(Ordering::SeqCst) as usize, data.len());
        let mut num_written = 0usize;
        while writable > 0 {
            for i in 0..writable {
                let tmp = &shared.ring_buf[(self.writer_idx + i) & RING_WRAP_MASK];
                
                *unsafe { &mut*tmp.get() } = Some(Vec::new());
                
                
                //data[num_written + i];
            }
            self.writer_idx += writable;
            num_written += writable; // Hmm, same operation on two integers?
            writable = usize::min(RING_WRAP_MASK - shared.num_available.fetch_add(writable as u32, Ordering::SeqCst) as usize + writable, data.len() - num_written);
        }
        num_written
    }
}

pub struct ByteChannel {
    reader: Reader,
    writer: Writer,
}

 */