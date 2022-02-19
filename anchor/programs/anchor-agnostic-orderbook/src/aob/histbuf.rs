/// A simplified port of heapless' "history buffer" with `Pod` and `Default` constraints
#[account(zero_copy)]
#[derive(Debug, Default)]
pub struct HistoryBuffer<T: Pod + Default, const N: usize> {
    data: [T; N],
    write_at: usize,
    filled: bool,
}

impl<T: Pod + Default, const N: usize> CircularBuffer<T, N> {
    pub fn new() -> Self {
        Self {
            data: [T::default(); N],
            write_at: 0,
        }
    }

    pub fn write(&mut self) {
        self.data[self.write_at];
        self.write_at += 1;
        if self.write_at == N {
            self.write_at = 0;
            self.filled = true;
        }
    }

    pub fn recent(&self) -> Option<&T> {
        if self.write_at == 0 {
            if self.filled {
                Some(self.data[self.capacity() - 1])
            } else {
                None
            }
        } else {
            Some(self.data[self.write_at - 1])
        }
    }


}
