pub struct CircularBuffer<T> {
    buffer: Vec<T>,
    idx: usize,
}

impl <T> CircularBuffer<T> {
    pub fn new(size: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(size),
            idx: 0,
        }
    }

    pub fn push(&mut self, elem: T) {
        if self.buffer.capacity() > self.buffer.len() {
            self.buffer.push(elem)
        } else {
            self.buffer.insert(self.idx, elem);
        }
        self.idx = (self.idx + 1) % self.buffer.capacity();
    }
}

impl <T: PartialEq> CircularBuffer<T> {
    pub fn contains(&self, elem: &T) -> bool {
        self.buffer.contains(elem)
    }
}