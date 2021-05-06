use libc;
use std::intrinsics::unlikely;
use std::fmt;
use std::fmt::Write;
use spin::Mutex;

#[doc(hidden)]
#[inline(never)]
pub fn _print_nl(args: fmt::Arguments<'_>) {
    let mut log = LOG.lock();
    log.write_fmt(args).unwrap();
    log.put_char('\n' as _);
    log.flush();
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        $crate::log::_print_nl(format_args!($($arg)*));
    }};
}

static LOG: Mutex<Log> = Mutex::new(Log::new());

struct Log {
    cursor: usize,
    buffer: [u8; 80],
}

impl Log {
    const fn new() -> Self {
        Self {
            cursor: 0,
            buffer: [0; 80],
        }
    }

    #[cold]
    fn flush(&mut self) {
        unsafe {
            libc::write(1, self.buffer.as_ptr() as _, self.cursor);
        }
        self.cursor = 0;
    }

    #[inline(always)]
    fn put_char(&mut self, c: u8) {
        self.buffer[self.cursor] = c;
        self.cursor += 1;
        if unlikely(self.cursor >= self.buffer.len()) {
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
