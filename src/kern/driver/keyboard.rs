use ::kern::arch::port::Port;
use core::sync::atomic::{AtomicBool, Ordering};
use ::kern::interrupts::idt::*;
use ::kern::interrupts::irq::PIC_CHAIN;
use spin::Mutex;
use ::kern::console::LogLevel::*;
use ::kern::console::{Console, tty1};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum KeyCode {
// Alphanumeric keys ////////////////
    KEY_INVALID           = 0,

    KEY_SPACE             = b' ' as u16,
    KEY_0                 = b'0' as u16,
    KEY_1                 = b'1' as u16,
    KEY_2                 = b'2' as u16,
    KEY_3                 = b'3' as u16,
    KEY_4                 = b'4' as u16,
    KEY_5                 = b'5' as u16,
    KEY_6                 = b'6' as u16,
    KEY_7                 = b'7' as u16,
    KEY_8                 = b'8' as u16,
    KEY_9                 = b'9' as u16,

    KEY_A                 = b'a' as u16,
    KEY_B                 = b'b' as u16,
    KEY_C                 = b'c' as u16,
    KEY_D                 = b'd' as u16,
    KEY_E                 = b'e' as u16,
    KEY_F                 = b'f' as u16,
    KEY_G                 = b'g' as u16,
    KEY_H                 = b'h' as u16,
    KEY_I                 = b'i' as u16,
    KEY_J                 = b'j' as u16,
    KEY_K                 = b'k' as u16,
    KEY_L                 = b'l' as u16,
    KEY_M                 = b'm' as u16,
    KEY_N                 = b'n' as u16,
    KEY_O                 = b'o' as u16,
    KEY_P                 = b'p' as u16,
    KEY_Q                 = b'q' as u16,
    KEY_R                 = b'r' as u16,
    KEY_S                 = b's' as u16,
    KEY_T                 = b't' as u16,
    KEY_U                 = b'u' as u16,
    KEY_V                 = b'v' as u16,
    KEY_W                 = b'w' as u16,
    KEY_X                 = b'x' as u16,
    KEY_Y                 = b'y' as u16,
    KEY_Z                 = b'z' as u16,

    KEY_DOT               = b'.' as u16,
    KEY_COMMA             = b',' as u16,
    KEY_COLON             = b':' as u16,
    KEY_SEMICOLON         = b';' as u16,
    KEY_SLASH             = b'/' as u16,
    KEY_BACKSLASH         = b'\\' as u16,
    KEY_PLUS              = b'+' as u16,
    KEY_MINUS             = b'-' as u16,
    KEY_ASTERISK          = b'*' as u16,
    KEY_EXCLAMATION       = b'!' as u16,
    KEY_QUESTION          = b'?' as u16,
    KEY_QUOTEDOUBLE       = b'\"' as u16,
    KEY_QUOTE             = b'\'' as u16,
    KEY_EQUAL             = b'=' as u16,
    KEY_HASH              = b'#' as u16,
    KEY_PERCENT           = b'%' as u16,
    KEY_AMPERSAND         = b'&' as u16,
    KEY_UNDERSCORE        = b'_' as u16,
    KEY_LEFTPARENTHESIS   = b'(' as u16,
    KEY_RIGHTPARENTHESIS  = b')' as u16,
    KEY_LEFTBRACKET       = b'[' as u16,
    KEY_RIGHTBRACKET      = b']' as u16,
    KEY_LEFTCURL          = b'{' as u16,
    KEY_RIGHTCURL         = b'}' as u16,
    KEY_DOLLAR            = b'$' as u16,
    KEY_LESS              = b'<' as u16,
    KEY_GREATER           = b'>' as u16,
    KEY_BAR               = b'|' as u16,
    KEY_GRAVE             = b'`' as u16,
    KEY_TILDE             = b'~' as u16,
    KEY_AT                = b'@' as u16,
    KEY_CARRET            = b'^' as u16,

    KEY_RETURN            = b'\r' as u16,
    KEY_BACKSPACE         = b'\x08' as u16,
    KEY_ESCAPE            = 0x1001,

// Arrow keys ////////////////////////

    KEY_UP                = 0x1100,
    KEY_DOWN              = 0x1101,
    KEY_LEFT              = 0x1102,
    KEY_RIGHT             = 0x1103,

// Function keys /////////////////////

    KEY_F1                = 0x1201,
    KEY_F2                = 0x1202,
    KEY_F3                = 0x1203,
    KEY_F4                = 0x1204,
    KEY_F5                = 0x1205,
    KEY_F6                = 0x1206,
    KEY_F7                = 0x1207,
    KEY_F8                = 0x1208,
    KEY_F9                = 0x1209,
    KEY_F10               = 0x120a,
    KEY_F11               = 0x120b,
    KEY_F12               = 0x120c,
    KEY_F13               = 0x120d,
    KEY_F14               = 0x120e,
    KEY_F15               = 0x120f,

// Numeric keypad //////////////////////

    KEY_KP_0              = 0x3001,
    KEY_KP_1              = 0x3002,
    KEY_KP_2              = 0x3003,
    KEY_KP_3              = 0x3004,
    KEY_KP_4              = 0x3005,
    KEY_KP_5              = 0x3006,
    KEY_KP_6              = 0x3007,
    KEY_KP_7              = 0x3008,
    KEY_KP_8              = 0x3009,
    KEY_KP_9              = 0x300a,
    KEY_KP_PLUS           = 0x300b,
    KEY_KP_MINUS          = 0x300c,
    KEY_KP_DECIMAL        = 0x300d,
    KEY_KP_DIVIDE         = 0x300e,
    KEY_KP_ASTERISK       = 0x300f,
    KEY_KP_NUMLOCK        = 0x3010,
    KEY_KP_ENTER          = 0x3011,

    KEY_TAB               = 0x4000,
    KEY_CAPSLOCK          = 0x4001,

// Modify keys ////////////////////////////

    KEY_LSHIFT            = 0x4002,
    KEY_LCTRL             = 0x4003,
    KEY_LALT              = 0x4004,
    KEY_LWIN              = 0x4005,
    KEY_RSHIFT            = 0x4006,
    KEY_RCTRL             = 0x4007,
    KEY_RALT              = 0x4008,
    KEY_RWIN              = 0x4009,

    KEY_INSERT            = 0x400a,
    KEY_DELETE            = 0x400b,
    KEY_HOME              = 0x400c,
    KEY_END               = 0x400d,
    KEY_PAGEUP            = 0x400e,
    KEY_PAGEDOWN          = 0x400f,
    KEY_SCROLLLOCK        = 0x4010,
    KEY_PAUSE             = 0x4011,

    KEY_UNKNOWN
}

bitflags! {
    flags KeyStatus: u16 {
        const KB_SCROLL_LOCK = 0x0001,
        const KB_NUM_LOCK = 0x0002,
        const KB_CAPS_LOCK = 0x0004,

        const KB_PRESS = 0x0010,
        const KB_RELEASE = 0x0020,

        const KB_SHIFT_DOWN = 0x0100,
        const KB_CTRL_DOWN = 0x0200,
        const KB_ALT_DOWN = 0x0400,
        const KB_META_DOWN = 0x0800,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct KeyPacket {
    keycode: KeyCode,
    status: u16,
}

#[allow(non_camel_case_types)]
pub enum MouseStatus {
    MOUSE_LEFT_DOWN = 0x0001,
    MOUSE_RIGHT_DOWN = 0x0002,
    MOUSE_MID_DOWN = 0x0004,
}

#[derive(Debug, Clone, Copy)]
pub struct MousePacket {
    flags: u16,
    relx: u16,
    rely: i16,
}

// KB_ENCODER_IO 
const KB_ENC_INPUT_BUF: u16 =   0x60;
const KB_ENC_CMD_REG: u16   =   0x60;

// KB_CTRL_IO 
const KB_CTRL_STATS_REG: u16 =   0x64;
const KB_CTRL_CMD_REG: u16   =   0x64;

bitflags! {
    flags KeyboardCtrlStatsMask: u8 {
        const STATS_MASK_OUT_BUF   =   0x01,
        const STATS_MASK_IN_BUF    =   0x02,
        const STATS_MASK_SYSTEM    =   0x04,
        const STATS_MASK_CMD_DATA  =   0x08,
        const STATS_MASK_LOCKED    =   0x10,
        const STATS_MASK_AUX_BUF   =   0x20,
        const STATS_MASK_TIMEOUT   =   0x40,
        const STATS_MASK_PARITY    =   0x80
    }
}

const KB_CTRL_STATS_OUT_BUF_EMPTY:u8 = 0;
const KB_CTRL_STATS_OUT_BUF_READY:u8 =  1;
const KB_CTRL_STATS_IN_BUF_READY:u8 = 0;
const KB_CTRL_STATS_IN_BUF_FULL:u8 =  1;

#[allow(non_camel_case_types)]
pub enum KeyboardEncoderCommand {
    SET_LEDS              =   0xED,
    ECHO                  =   0xEE,
    SCAN_CODE_SET         =   0xF0,
    ID                    =   0xF2,
    AUTODELAY             =   0xF3,
    ENABLE                =   0xF4,
    RESETWAIT             =   0xF5,
    RESETSCAN             =   0xF6,
    ALL_AUTO              =   0xF7,
    ALL_MAKEBREAK         =   0xF8,
    ALL_MAKEONLY          =   0xF9,
    ALL_MAKEBREAK_AUTO    =   0xFA,
    SINGLE_AUTOREPEAT     =   0xFB,
    SINGLE_MAKEBREAK      =   0xFC,
    SINGLE_BREAKONLY      =   0xFD,
    RESEND                =   0xFE,
    RESET                 =   0xFF
}


#[allow(non_camel_case_types)]
pub enum KeyboardCtrlCommand {
    READ             =   0x20,
    WRITE            =   0x60,
    SELF_TEST        =   0xAA,
    INTERFACE_TEST   =   0xAB,
    DISABLE          =   0xAD,
    ENABLE           =   0xAE,
    READ_IN_PORT     =   0xC0,
    READ_OUT_PORT    =   0xD0,
    WRITE_OUT_PORT   =   0xD1,
    READ_TEST_INPUTS =   0xE0,
    SYSTEM_RESET     =   0xFE,
    MOUSE_DISABLE    =   0xA7,
    MOUSE_ENABLE     =   0xA8,
    MOUSE_PORT_TEST  =   0xA9,
    MOUSE_WRITE      =   0xD4
}

#[allow(non_camel_case_types)]
enum KyeboardError {
    BUF_OVERRUN           =   0,
    ID_RET                =   0x83AB,
    BAT                   =   0xAA,   //note: can also be L. shift key make code
    ECHO_RET              =   0xEE,
    ACK                   =   0xFA,
    BAT_FAILED            =   0xFC,
    DIAG_FAILED           =   0xFD,
    RESEND_CMD            =   0xFE,
    KEY                   =   0xFF
}


// original xt scan code set 1. Array index==make code
// change what keys the scan code correspond to if your scan code set is different
static _xtkb_scancode_std: [KeyCode; 89] = [
    /// key         scancode
    KeyCode::KEY_UNKNOWN,    //0
    KeyCode::KEY_ESCAPE,     //1
    KeyCode::KEY_1,          //2
    KeyCode::KEY_2,          //3
    KeyCode::KEY_3,          //4
    KeyCode::KEY_4,          //5
    KeyCode::KEY_5,          //6
    KeyCode::KEY_6,          //7
    KeyCode::KEY_7,          //8
    KeyCode::KEY_8,          //9
    KeyCode::KEY_9,          //0xa
    KeyCode::KEY_0,          //0xb
    KeyCode::KEY_MINUS,      //0xc
    KeyCode::KEY_EQUAL,      //0xd
    KeyCode::KEY_BACKSPACE,  //0xe
    KeyCode::KEY_TAB,        //0xf

    KeyCode::KEY_Q,          //0x10
    KeyCode::KEY_W,          //0x11
    KeyCode::KEY_E,          //0x12
    KeyCode::KEY_R,          //0x13
    KeyCode::KEY_T,          //0x14
    KeyCode::KEY_Y,          //0x15
    KeyCode::KEY_U,          //0x16
    KeyCode::KEY_I,          //0x17
    KeyCode::KEY_O,          //0x18
    KeyCode::KEY_P,          //0x19
    KeyCode::KEY_LEFTBRACKET,//0x1a
    KeyCode::KEY_RIGHTBRACKET,//0x1b
    KeyCode::KEY_RETURN,     //0x1c
    KeyCode::KEY_LCTRL,      //0x1d
    KeyCode::KEY_A,          //0x1e
    KeyCode::KEY_S,          //0x1f

    KeyCode::KEY_D,          //0x20
    KeyCode::KEY_F,          //0x21
    KeyCode::KEY_G,          //0x22
    KeyCode::KEY_H,          //0x23
    KeyCode::KEY_J,          //0x24
    KeyCode::KEY_K,          //0x25
    KeyCode::KEY_L,          //0x26
    KeyCode::KEY_SEMICOLON,  //0x27
    KeyCode::KEY_QUOTE,      //0x28
    KeyCode::KEY_GRAVE,      //0x29
    KeyCode::KEY_LSHIFT,     //0x2a
    KeyCode::KEY_BACKSLASH,  //0x2b
    KeyCode::KEY_Z,          //0x2c
    KeyCode::KEY_X,          //0x2d
    KeyCode::KEY_C,          //0x2e
    KeyCode::KEY_V,          //0x2f

    KeyCode::KEY_B,          //0x30
    KeyCode::KEY_N,          //0x31
    KeyCode::KEY_M,          //0x32
    KeyCode::KEY_COMMA,      //0x33
    KeyCode::KEY_DOT,        //0x34
    KeyCode::KEY_SLASH,      //0x35
    KeyCode::KEY_RSHIFT,     //0x36
    KeyCode::KEY_KP_ASTERISK,//0x37
    KeyCode::KEY_LALT,       //0x38
    KeyCode::KEY_SPACE,      //0x39
    KeyCode::KEY_CAPSLOCK,   //0x3a
    KeyCode::KEY_F1,         //0x3b
    KeyCode::KEY_F2,         //0x3c
    KeyCode::KEY_F3,         //0x3d
    KeyCode::KEY_F4,         //0x3e
    KeyCode::KEY_F5,         //0x3f

    KeyCode::KEY_F6,         //0x40
    KeyCode::KEY_F7,         //0x41
    KeyCode::KEY_F8,         //0x42
    KeyCode::KEY_F9,         //0x43
    KeyCode::KEY_F10,        //0x44
    KeyCode::KEY_KP_NUMLOCK, //0x45
    KeyCode::KEY_SCROLLLOCK, //0x46
    KeyCode::KEY_HOME,       //0x47
    KeyCode::KEY_KP_8,       //0x48  //keypad up arrow
    KeyCode::KEY_PAGEUP,     //0x49
    KeyCode::KEY_KP_MINUS,   //0x4a
    KeyCode::KEY_KP_4,       //0x4b
    KeyCode::KEY_KP_5,       //0x4c
    KeyCode::KEY_KP_6,       //0x4d
    KeyCode::KEY_KP_PLUS,    //0x4e
    KeyCode::KEY_KP_1,       //0x4f

    KeyCode::KEY_KP_2,       //0x50  //keypad down arrow
    KeyCode::KEY_KP_3,       //0x51  //keypad page down
    KeyCode::KEY_KP_0,       //0x52  //keypad insert key
    KeyCode::KEY_KP_DECIMAL, //0x53  //keypad delete key
    KeyCode::KEY_UNKNOWN,    //0x54
    KeyCode::KEY_UNKNOWN,    //0x55
    KeyCode::KEY_UNKNOWN,    //0x56
    KeyCode::KEY_F11,        //0x57
    KeyCode::KEY_F12         //0x58
];

#[derive(Debug, Clone, Copy)]
struct ExtendScancode {
    scan: u8,
    keycode: KeyCode,
}

static _xtkb_scancode_ex: [ExtendScancode; 10] = [
    ExtendScancode {scan: 0x1c, keycode: KeyCode::KEY_KP_ENTER},
    ExtendScancode {scan: 0x1d, keycode: KeyCode::KEY_RCTRL},
    ExtendScancode {scan: 0x35, keycode: KeyCode::KEY_KP_DIVIDE},
    ExtendScancode {scan: 0x38, keycode: KeyCode::KEY_RALT},
    ExtendScancode {scan: 0x47, keycode: KeyCode::KEY_HOME},
    ExtendScancode {scan: 0x4f, keycode: KeyCode::KEY_END},
    ExtendScancode {scan: 0x51, keycode: KeyCode::KEY_PAGEDOWN},
    ExtendScancode {scan: 0x52, keycode: KeyCode::KEY_INSERT},
    ExtendScancode {scan: 0x53, keycode: KeyCode::KEY_DELETE},
    ExtendScancode {scan: 0x9c, keycode: KeyCode::KEY_KP_ENTER},
];

fn get_extend_keycode(data: u8) -> KeyCode {
    for xs in _xtkb_scancode_ex.iter() {
        if xs.scan == data & 0x7f {
            return xs.keycode;
        }
    }
    return KeyCode::KEY_UNKNOWN;
}

static _is_extended: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub struct Keyboard {
    encoder: Port<u8>,
    ctrl: Port<u8>,
    status: Option<KeyStatus>,
    //kb_buf: KeyBuffer,
}

pub static KBD: Mutex<Keyboard> = Mutex::new(Keyboard::new());

impl Keyboard {
    pub const fn new() -> Keyboard {
        Keyboard {
            encoder: Port::new(KB_ENC_CMD_REG),
            ctrl: Port::new(KB_CTRL_CMD_REG),
            status: None
        }
    }


    // 0: ok
    // 1: resend
    // 2: bad
    fn check_reply(&mut self) -> i8 {
        let d = self.kbc_read();
        match d {
            0xFA => 0, //ACK
            0xFE => 1,
            _ => 2
        }
    }

    //FIXME: need to check if mouse exists and this won't work 
    //if got usb mouse. (no usb bus configured)
    pub fn init(&mut self) {
        //self.kbc_send(KeyboardCtrlCommand::MOUSE_ENABLE);
        // enable mouse data reporting
        //self.kbc_send(KeyboardCtrlCommand::MOUSE_WRITE);
        self.kbe_send(KeyboardEncoderCommand::ENABLE as u8);
        self.check_reply();

        self.kbc_send(KeyboardCtrlCommand::WRITE as u8);
        self.kbe_send(0x47);
        self.check_reply();

        self.set_leds(false, false, false);
        //register_isr_handler(IRQ_KBD, keyboard_irq);
        //register_isr_handler(IRQ_MOUSE, mouse_irq);
    }

    fn kbe_send(&mut self, cmd: u8) {
        self.poll_aux_status();
        self.encoder.write(cmd);
    }

    fn kbc_send(&mut self, cmd: u8) {
        self.poll_aux_status();
        self.ctrl.write(cmd);
    }

    fn poll_aux_status(&mut self) -> bool {
        let mut retries = 0;
        while (self.kbc_read() & 0x03) != 0 && retries < 60 {
            if self.can_read() {
                self.kbe_read();
            }
            retries += 1;
        }

        return retries != 60;
    }

    fn can_write(&mut self) -> bool {
        let status = KeyboardCtrlStatsMask::from_bits(self.kbc_read());
        status.map_or(false, |st| st.contains(STATS_MASK_IN_BUF))
    }

    fn can_read(&mut self) -> bool {
        let status = KeyboardCtrlStatsMask::from_bits(self.kbc_read());
        status.map_or(false, |st| st.contains(STATS_MASK_OUT_BUF))
    }

    fn kbe_wait_and_read(&mut self) -> u8 {
        loop {
            if self.can_read() {
                break;
            }
        }
        self.kbe_read()
    }

    fn kbe_read(&mut self) -> u8 {
        self.encoder.read()
    }

    fn kbc_read(&mut self) -> u8 {
        self.ctrl.read()
    }

    //Bit 0: Scroll lock LED (0: off 1:on)
    //Bit 1: Num lock LED (0: off 1:on)
    //Bit 2: Caps lock LED (0: off 1:on)
    fn set_leds(&mut self, scroll: bool, num: bool, caps: bool) {
        let mut status = self.status.map_or(KeyStatus::empty(), |st| st);
        if scroll {
            status |= KB_SCROLL_LOCK;
        }
        if num {
            status |= KB_NUM_LOCK;
        }
        if caps {
            status |= KB_CAPS_LOCK;
        }

        if status.is_empty() {
            self.status = None;
        } else {
            self.status = Some(status);
        }
        let data = status.bits();
        self.kbe_send(KeyboardEncoderCommand::SET_LEDS as u8);
        self.kbe_send(data as u8);
    }

    fn set_shift_down(&mut self, val: bool) {
        if let Some(ref mut st) = self.status {
            if val {
                st.insert(KB_SHIFT_DOWN);
            } else {
                st.remove(KB_SHIFT_DOWN);
            }
        }
    }

    fn set_ctrl_down(&mut self, val: bool) {
        if let Some(ref mut st) = self.status {
            if val {
                st.insert(KB_CTRL_DOWN);
            } else {
                st.remove(KB_CTRL_DOWN);
            }
        }
    }

    fn set_alt_down(&mut self, val: bool) {
        if let Some(ref mut st) = self.status {
            if val {
                st.insert(KB_ALT_DOWN);
            } else {
                st.remove(KB_ALT_DOWN);
            }
        }
    }

    fn alt_down(&mut self) -> bool {
        self.status.map_or(false, |st| st.contains(KB_ALT_DOWN))
    }

    fn ctrl_down(&mut self) -> bool {
        self.status.map_or(false, |st| st.contains(KB_CTRL_DOWN))
    }

    fn shift_down(&mut self) -> bool {
        self.status.map_or(false, |st| st.contains(KB_SHIFT_DOWN))
    }
}

//extern void tty_enqueue();
//FIXME: stub here, need tty later
fn tty_enqueue() {}

impl KeyCode {
    fn printable(&self) -> bool {
        match *self {
            KeyCode::KEY_SPACE | KeyCode::KEY_0 | KeyCode::KEY_1 | KeyCode::KEY_2 |
                KeyCode::KEY_3 | KeyCode::KEY_4 | KeyCode::KEY_5 | KeyCode::KEY_6 | 
                KeyCode::KEY_7 | KeyCode::KEY_8 | KeyCode::KEY_9 | KeyCode::KEY_A |
                KeyCode::KEY_B | KeyCode::KEY_C | KeyCode::KEY_D | KeyCode::KEY_E | 
                KeyCode::KEY_F | KeyCode::KEY_G | KeyCode::KEY_H | KeyCode::KEY_I | 
                KeyCode::KEY_J | KeyCode::KEY_K | KeyCode::KEY_L | KeyCode::KEY_M | 
                KeyCode::KEY_N | KeyCode::KEY_O | KeyCode::KEY_P | KeyCode::KEY_Q | 
                KeyCode::KEY_R | KeyCode::KEY_S | KeyCode::KEY_T | KeyCode::KEY_U | 
                KeyCode::KEY_V | KeyCode::KEY_W | KeyCode::KEY_X | KeyCode::KEY_Y | 
                KeyCode::KEY_Z | KeyCode::KEY_DOT | KeyCode::KEY_COMMA | KeyCode::KEY_COLON | 
                KeyCode::KEY_SEMICOLON | KeyCode::KEY_SLASH | KeyCode::KEY_BACKSLASH | 
                KeyCode::KEY_PLUS | KeyCode::KEY_MINUS | KeyCode::KEY_ASTERISK |
                KeyCode::KEY_EXCLAMATION | KeyCode::KEY_QUESTION | KeyCode::KEY_QUOTEDOUBLE |
                KeyCode::KEY_QUOTE | KeyCode::KEY_EQUAL | KeyCode::KEY_HASH | 
                KeyCode::KEY_PERCENT | KeyCode::KEY_AMPERSAND | KeyCode::KEY_UNDERSCORE |
                KeyCode::KEY_LEFTPARENTHESIS | KeyCode::KEY_RIGHTPARENTHESIS | 
                KeyCode::KEY_LEFTBRACKET | KeyCode::KEY_RIGHTBRACKET | KeyCode::KEY_LEFTCURL |
                KeyCode::KEY_RIGHTCURL | KeyCode::KEY_DOLLAR | KeyCode::KEY_LESS | 
                KeyCode::KEY_GREATER | KeyCode::KEY_BAR | KeyCode::KEY_GRAVE |
                KeyCode::KEY_TILDE | KeyCode::KEY_AT | KeyCode::KEY_CARRET | 
                KeyCode::KEY_RETURN | KeyCode::KEY_BACKSPACE => true,
            _ => false
        }
    }
}

//FIXME: I use KBD (spin)lock here, so there might be a deadlock
pub extern "C" fn keyboard_irq(frame: &mut ExceptionStackFrame) {
    unsafe {
        PIC_CHAIN.lock().eoi(0);
    }
    let mut kbd = KBD.lock();
    let data = kbd.kbe_wait_and_read();

    //printk!(Info, "keypress!\n\r");

    if data == 0xE0 {
        _is_extended.store(true, Ordering::Relaxed);
        return;
    }

    let mut packet = KeyPacket {
        keycode: KeyCode::KEY_UNKNOWN,
        status: 0
    };

    let extended = _is_extended.load(Ordering::Relaxed);
    if data & 0x80 != 0 {
        packet.status = KB_RELEASE.bits();
         packet.keycode = match extended {
            true => get_extend_keycode(data),
            false if ((data & 0x7f) as usize) < _xtkb_scancode_std.len() =>
                _xtkb_scancode_std[(data & 0x7f) as usize],
            _ => {
                printk!(Warn, "weird scancode {}", data);
                KeyCode::KEY_UNKNOWN
            }
        };

        //Break Code
        match packet.keycode {
            KeyCode::KEY_LSHIFT | KeyCode::KEY_RSHIFT => kbd.set_shift_down(false), 
            KeyCode::KEY_LCTRL | KeyCode::KEY_RCTRL => kbd.set_ctrl_down(false), 
            KeyCode::KEY_LALT | KeyCode::KEY_RALT => kbd.set_alt_down(false), 
            _ => {}
        }

    } else {
        packet.status = KB_PRESS.bits();
        packet.keycode = match extended {
            true =>  get_extend_keycode(data),
            false if (data as usize) < _xtkb_scancode_std.len() => _xtkb_scancode_std[data as usize],
            _ => {
                printk!(Warn, "weird scancode {}", data);
                KeyCode::KEY_UNKNOWN
            }
        };

        //Make Code
        match packet.keycode {
            KeyCode::KEY_LSHIFT | KeyCode::KEY_RSHIFT => kbd.set_shift_down(true),
            KeyCode::KEY_LCTRL | KeyCode::KEY_RCTRL => kbd.set_ctrl_down(true),
            KeyCode::KEY_LALT | KeyCode::KEY_RALT => kbd.set_alt_down(true),
            _ => {}
        }
    }

    if kbd.shift_down() {
        match packet.keycode {
            KeyCode::KEY_0 =>             packet.keycode = KeyCode::KEY_RIGHTPARENTHESIS,
            KeyCode::KEY_1 =>             packet.keycode = KeyCode::KEY_EXCLAMATION,
            KeyCode::KEY_2 =>             packet.keycode = KeyCode::KEY_AT,
            KeyCode::KEY_3 =>             packet.keycode = KeyCode::KEY_HASH,
            KeyCode::KEY_4 =>             packet.keycode = KeyCode::KEY_DOLLAR,
            KeyCode::KEY_5 =>             packet.keycode = KeyCode::KEY_PERCENT,
            KeyCode::KEY_6 =>             packet.keycode = KeyCode::KEY_CARRET,
            KeyCode::KEY_7 =>             packet.keycode = KeyCode::KEY_AMPERSAND,
            KeyCode::KEY_8 =>             packet.keycode = KeyCode::KEY_ASTERISK,
            KeyCode::KEY_9 =>             packet.keycode = KeyCode::KEY_LEFTPARENTHESIS,
            KeyCode::KEY_UNDERSCORE =>    packet.keycode = KeyCode::KEY_MINUS,
            KeyCode::KEY_EQUAL =>         packet.keycode = KeyCode::KEY_PLUS,
            KeyCode::KEY_GRAVE =>         packet.keycode = KeyCode::KEY_TILDE,
            KeyCode::KEY_COMMA =>         packet.keycode = KeyCode::KEY_LESS,
            KeyCode::KEY_DOT =>           packet.keycode = KeyCode::KEY_GREATER,
            KeyCode::KEY_SLASH =>         packet.keycode = KeyCode::KEY_QUESTION,
            KeyCode::KEY_LEFTBRACKET =>   packet.keycode = KeyCode::KEY_LEFTCURL,
            KeyCode::KEY_RIGHTBRACKET =>  packet.keycode = KeyCode::KEY_RIGHTCURL,
            KeyCode::KEY_BACKSLASH =>     packet.keycode = KeyCode::KEY_BAR,
            KeyCode::KEY_QUOTE =>         packet.keycode = KeyCode::KEY_QUOTEDOUBLE,
            _ => {}
        }
    }
    packet.status |= kbd.status.map_or(0, |st| st.bits());

    let st = KeyStatus::from_bits(packet.status);
    if st.is_some() && st.unwrap().contains(KB_PRESS) && packet.keycode.printable() {
        print!("{}", packet.keycode as u8 as char);
    }
    //kbd.kbbuf().write(packet);
    tty_enqueue();

    if extended { _is_extended.store(false, Ordering::Relaxed); }
}

