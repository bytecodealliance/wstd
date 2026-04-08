//! Random number generation.

use wasip2::random;

/// Fill the slice with cryptographically secure random bytes.
pub fn get_random_bytes(buf: &mut [u8]) {
    match buf.len() {
        0 => {}
        _ => {
            let output = random::random::get_random_bytes(buf.len() as u64);
            buf.copy_from_slice(&output[..]);
        }
    }
}

/// Fill the slice with insecure random bytes.
pub fn get_insecure_random_bytes(buf: &mut [u8]) {
    match buf.len() {
        0 => {}
        _ => {
            let output = random::insecure::get_insecure_random_bytes(buf.len() as u64);
            buf.copy_from_slice(&output[..]);
        }
    }
}
