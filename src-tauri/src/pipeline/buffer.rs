use std::mem;

/// A thread-safe, double-buffered data container.
pub struct DoubleBuffer<T> {
    front: T,
    back: T,
}

impl<T: Default> DoubleBuffer<T> {
    pub fn new() -> Self {
        Self {
            front: T::default(),
            back: T::default(),
        }
    }
    
    /// Provides mutable access to the back buffer for writing.
    /// After the closure returns, the back buffer is swapped with the front.
    pub fn write_with<F>(&mut self, writer: F)
    where
        F: FnOnce(&mut T),
    {
        writer(&mut self.back);
        self.swap();
    }

    /// Provides read-only access to the front buffer.
    pub fn read_with<F, R>(&self, reader: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        reader(&self.front)
    }

    fn swap(&mut self) {
        mem::swap(&mut self.front, &mut self.back);
    }
}