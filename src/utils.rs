pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    } else if bytes >= 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / 1024.0 / 1024.0)
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}


// circular bufer for CPU history
pub struct CircularBuffer<T> {
    data: Vec<T>,
    capacity: usize,
    head: usize,
    size: usize,
}

/*
    i think somehow this uses more memory LMAO
    but okay ill do checks later in release mode and note them here
    PARAMS: 2000ms refresh -> not opening or closing any processes -> switching prioritizations 
    -> test thrice

    Vec:
        0.53% cpu avg.
        0.7% cpu 1% highs
        6.50mb usage

    CircBuffer:
        0.48%  cpu avg.
        0.56% cpu 1% highs
        5.12mb usage

    Summary:
        memory diff: -21.23%
        cpu avg diff: -9.43%
        cpu 1% highs diff: -20.0%

    i mean, was it really worth it? kind of. seems through testing that sometimes the new impl. does lead to the occasional rise of cpu usage but often tanks way down to 0.4x% every so
    often while using the Vec method kept a consistent 0.53%~
    will continue testing but it looks okay to merge for now

    caching made it worse for memory of course, just by a tad.

    just merge this stupid branch, adding rows caching increased the cpu usage by 1.7% and memory by 2.47% but it seems to be the better approach here.
*/
impl<T: Copy + Default> CircularBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: vec![T::default(); capacity],
            capacity,
            head: 0,
            size: 0,
        }
    }

    pub fn push(&mut self, value: T) {
        self.data[self.head] = value;
        self.head = (self.head + 1) % self.capacity;
        if self.size < self.capacity {
            self.size += 1;
        }
    }

    pub fn get(&self, index: usize) -> Option<T> {
        if index >= self.size {
            return None;
        }
        
        // calculate the actual index in the circular buffer
        let actual_index = if self.size < self.capacity {
            index
        } else {
            (self.head + index) % self.capacity
        };
        
        Some(self.data[actual_index])
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    // get all values in chronological order (oldest to newest)
    pub fn to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.size);
        for i in 0..self.size {
            if let Some(value) = self.get(i) {
                result.push(value);
            }
        }
        result
    }

    // get the latest values as a slice for direct use with sparkline
    pub fn as_slice(&self) -> &[T] {
        if self.size < self.capacity {
            &self.data[0..self.size]
        } else {
            // When buffer is full, we need to return data in correct order
            // TODO -> to_vec() will work for now
            &self.data
        }
    }
}
