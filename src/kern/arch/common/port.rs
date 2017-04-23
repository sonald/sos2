use core::marker::PhantomData;

use super::io::*;

pub trait IO {
    unsafe fn port_in(port: u16) -> Self;
    unsafe fn port_out(port: u16, val: Self);
}

impl IO for u8 {
    unsafe fn port_in(port: u16) -> Self {
        inb(port)
    }
    unsafe fn port_out(port: u16, val: Self) {
        outb(port, val);
    }
}

impl IO for u16 {
    unsafe fn port_in(port: u16) -> Self {
        inw(port)
    }
    unsafe fn port_out(port: u16, val: Self) {
        outw(port, val);
    }
}

impl IO for u32 {
    unsafe fn port_in(port: u16) -> Self {
        inl(port)
    }
    unsafe fn port_out(port: u16, val: Self) {
        outl(port, val);
    }
}


#[derive(Debug, Clone, Copy)]
pub struct Port<T: IO> {
    pub port: u16,
    phantom: PhantomData<T>
}

impl<T> Port<T> where T: IO {
    pub const fn new(port: u16) -> Port<T> {
        Port {
            port: port,
            phantom: PhantomData
        }
    }

    pub fn read(&self) -> T {
        unsafe { T::port_in(self.port) }
    }

    pub fn write(&mut self, v: T) {
        unsafe { T::port_out(self.port, v); }
    }
}


#[derive(Debug, Clone, Copy)]
pub struct UnsafePort<T: IO> {
    pub port: u16,
    phantom: PhantomData<T>
}

impl<T> UnsafePort<T> where T: IO {
    pub const unsafe fn new(port: u16) -> UnsafePort<T> {
        UnsafePort {
            port: port,
            phantom: PhantomData
        }
    }

    pub unsafe fn read(&self) -> T {
        T::port_in(self.port) 
    }

    pub unsafe fn write(&mut self, v: T) {
        T::port_out(self.port, v);
    }
}
