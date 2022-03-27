use std::os::raw::c_char;

#[cfg(not(feature = "tracing"))]
pub use log::{debug, error, info, trace, warn};

#[cfg(feature = "tracing")]
pub use tracing::{debug, error, info, trace, warn};

pub(crate) unsafe fn streq(mut a: *const c_char, mut b: *const c_char) -> bool {
    while *a == *b {
        if *a == 0 {
            return true;
        }
        a = a.add(1);
        b = b.add(1);
    }
    false
}
