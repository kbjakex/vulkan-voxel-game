use std::collections::VecDeque;

use crate::TICKS_PER_SECOND;

// 1.5 ticks
pub const DELAY_MS : u32 = 1500 / TICKS_PER_SECOND;

// Basically copied from https://github.com/Ralith/hypermine/blob/master/server/src/input_queue.rs 
// Thanks Ralith!

pub struct JitterPrevention<T> {
    entries: VecDeque<T>,
    time_thresh_ms: Option<u32>
}

impl<T> JitterPrevention<T> {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            time_thresh_ms: None
        }
    }

    pub fn push(&mut self, entry: T, time_ms: u32) {
        self.entries.push_back(entry);
        if self.time_thresh_ms.is_none() {
            self.time_thresh_ms = Some(time_ms);
        }
        //println!("Entries: {}", self.entries.len());
    }

    pub fn pop(&mut self, time_ms: u32, delay_ms: u32) -> Option<T> {
        if time_ms - self.time_thresh_ms? < delay_ms {
            return None;
        }
        let result = self.entries.pop_front();
        if result.is_none() {
            println!("OOPS");
            self.time_thresh_ms = None;
        }
        result
    }
}
