use super::frame::{Frame, FrameAllocator};
use super::PAGE_SIZE;

use ::kern::console as con;
use con::LogLevel::*;

use collections::Vec;

const UNIT: usize = PAGE_SIZE;
#[derive(Debug)]
pub struct BuddyAllocator {
    pub start: usize,
    pub size: usize,
    tree: Vec<usize>
}

impl BuddyAllocator {
    pub fn new(start: usize, size: usize) -> BuddyAllocator {
        let size = size / UNIT;
        let size = if size.is_power_of_two() { size } else { size.next_power_of_two() / 2 };
        let mut tree = vec![0; size*2];

        for i in 1..tree.len() {
            tree[i] = size * 2 / (i+1).next_power_of_two();
        }
        printk!(Info, "BuddyAllocator {:x}\n", tree.len() * ::core::mem::size_of::<usize>());
        BuddyAllocator { start, size, tree }
    }

    pub fn dump(&self) {
        for i in 1..self.tree.len() {
            if i.is_power_of_two() {
                printk!(Debug, "\n");
            }
            printk!(Debug, "{} ", self.tree[i]);
        }
        printk!(Debug, "\n");
    }

    fn address_of(&self, n: usize) -> Option<usize> {
        if n >= self.tree.len() {
            None
        } else {
            let o = (n+1).next_power_of_two() / 2;
            let chunk = self.size / o;
            let offset = chunk * (n - o);
            return Some(self.start + offset * UNIT);
        }
    }

    fn mark_used(&mut self, mut n: usize, size: usize) {
        self.tree[n] = 0;
        loop {
            n = n / 2;
            if n <= 0 { break }
            self.tree[n] = ::core::cmp::max(self.tree[n*2], self.tree[n*2+1]);
        }
    }

    pub fn alloc(&mut self, size: usize) -> Option<usize> {
        let size = (size / UNIT).next_power_of_two();
        if size == 0 || size > self.tree[1] {
            return None;
        }

        let mut n = 1;
        let mut sz = self.size;
        while sz > size {
            n = if self.tree[n*2] >= size {
                n*2
            } else {
                n*2+1
            };
            sz /= 2;
        }

        self.mark_used(n, size);
        self.address_of(n)
    }

    pub fn dealloc(&mut self, addr: usize) {
        if addr < self.start { return; }

        let mut offset = (addr - self.start) / UNIT;
        if offset >= self.size { return; }

        let mut size = 1;
        let mut n = offset + self.size;

        while self.tree[n] != 0 {
            n = n / 2;
            size *= 2;
        }

        self.tree[n] = size;
        while n > 1 {
            n /= 2;
            size *= 2;

            let l = self.tree[n*2];
            let r = self.tree[n*2+1];
            self.tree[n] = if l + r == size {
                size
            } else {
                ::core::cmp::max(l, r)
            };
        }
    }
}

impl FrameAllocator for BuddyAllocator {
    fn alloc_frame(&mut self) -> Option<Frame> {
        self.alloc(UNIT).map(|addr| Frame::from_paddress(addr))
    }

    fn dealloc_frame(&mut self, frame: Frame) {
        //printk!(Debug, "dealloc {:#x}\n", frame.start_address());
        self.dealloc(frame.start_address())
    }
}
