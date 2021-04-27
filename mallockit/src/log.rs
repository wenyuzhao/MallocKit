use libc;
use std::fmt;
use std::fmt::Write;
use spin::Mutex;

#[doc(hidden)]
#[inline(never)]
pub fn _print_nl(args: fmt::Arguments<'_>) {
    let mut log = LOG.lock();
    log.write_fmt(args).unwrap();
    log.putc('\n' as _);
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        $crate::log::_print_nl(format_args!($($arg)*));
    }};
}

static LOG: Mutex<Log> = Mutex::new(Log);

struct Log;

impl Log {
    fn putc(&self, c: i32) {
        static mut BUF: [i32; 1] = [0; 1];
        unsafe {
            BUF[0] = c;
            libc::write(1, BUF.as_ptr() as _, 1);
        }
    }
}

impl Write for Log {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        for b in s.bytes() {
            self.putc(b as _);
        }
        Ok(())
    }
}
