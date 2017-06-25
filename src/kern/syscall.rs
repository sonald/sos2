use ::kern::console::LogLevel::*;
use ::kern::task;
use ::kern::arch::cpu;
use ::kern::console::{Console, tty1};

use core::sync::atomic::Ordering;
use x86_64::instructions::interrupts;

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum Syscall {
    NONE          =   0,
    FORK          =   1,
    EXIT          =   2,
    WAIT          =   3,
    PIPE          =   4,
    READ          =   5,
    KILL          =   6,
    EXEC          =   7,
    FSTAT         =   8,
    CHDIR         =   9,
    DUP           =  10,
    GETPID        =  11,
    SBRK          =  12,
    SLEEP         =  13,
    UPTIME        =  14,
    OPEN          =  15,
    WRITE         =  16,
    MKNOD         =  17,
    UNLINK        =  18,
    LINK          =  19,
    MKDIR         =  20,
    CLOSE         =  21,
    MOUNT         =  22,
    UMOUNT        =  23,
    GETPPID       =  24,
    MMAP          =  25,
    READDIR       =  26,
    DUP2          =  27,
    KDUMP         =  28,
    LSEEK         =  29,
    STAT          =  30,
    LSTAT         =  31,
    SIGNAL        =  32,
    SIGACTION     =  33,
    SIGPENDING    =  34,
    SIGPROCMASK   =  35,
    SIGSUSPEND    =  36,
    SIGRETURN     =  37,
    WAITPID       =  38,
    FCHDIR        =  39,
    GETCWD        =  40,

    NR_SYSCALL    =  41
}

#[no_mangle]
pub unsafe extern "C" fn syscall_dispatch(id: usize, args: *const usize) 
{
    let args = ::core::slice::from_raw_parts(args, 6);
    let tid = task::CURRENT_ID.load(Ordering::SeqCst);
    Console::with(&tty1, 19, 0, || {
        printk!(Info, "syscall({}) tid {}: {} {} {} {} {} {}\n\r", id, tid, 
                args[0], args[1], args[2], args[3], args[4], args[5]);
    });

    if id == Syscall::NONE as usize || id > Syscall::NR_SYSCALL as usize {
        panic!("invalid syscall id {}", id);
    }

    let nr: Syscall = ::core::intrinsics::transmute(id);
    match nr {
        Syscall::WRITE => {
            let buf = ::core::slice::from_raw_parts(args[1] as *const u8, args[2]);
            sys_write(args[0] as isize, buf);
        },
        _ => {
            unimplemented!()
        }
    }
}


pub fn init()
{
    
}

pub fn sys_write(fd: isize, buf: &[u8]) {
    let msg = ::core::str::from_utf8(buf).unwrap();
    Console::with(&tty1, 18, 0, || { printk!(Debug, "sys_write {}\n\r", msg); });
}

