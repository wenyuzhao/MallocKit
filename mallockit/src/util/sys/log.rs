use spin::Mutex;
use std::fmt;
use std::fmt::Write;

#[doc(hidden)]
#[cold]
pub fn _print(args: fmt::Arguments<'_>, new_line: bool, stderr: bool) {
    let mut log = if stderr { ERR.lock() } else { LOG.lock() };
    log.write_fmt(args).unwrap();
    if new_line {
        log.put_char(b'\n');
    }
    log.flush();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::util::sys::log::_print(format_args!($($arg)*), false, false);
    }};
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        $crate::util::sys::log::_print(format_args!($($arg)*), true, false);
    }};
}

#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => {{
        $crate::util::sys::log::_print(format_args!($($arg)*), false, true);
    }};
}

#[macro_export]
macro_rules! eprintln {
    ($($arg:tt)*) => {{
        $crate::util::sys::log::_print(format_args!($($arg)*), true, true);
    }};
}

static LOG: Mutex<Log> = Mutex::new(Log::new(false));
static ERR: Mutex<Log> = Mutex::new(Log::new(true));

struct Log {
    stderr: bool,
    cursor: usize,
    buffer: [u8; 80],
}

impl Log {
    const fn new(stderr: bool) -> Self {
        Self {
            stderr,
            cursor: 0,
            buffer: [0; 80],
        }
    }

    #[cold]
    fn flush(&mut self) {
        unsafe {
            if self.stderr {
                libc::write(2, self.buffer.as_ptr() as _, self.cursor);
            } else {
                libc::write(1, self.buffer.as_ptr() as _, self.cursor);
            }
        }
        self.cursor = 0;
    }

    fn put_char(&mut self, c: u8) {
        self.buffer[self.cursor] = c;
        self.cursor += 1;
        if self.cursor >= self.buffer.len() {
            self.flush();
        }
    }
}

impl Write for Log {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        for b in s.bytes() {
            self.put_char(b);
        }
        Ok(())
    }
}
