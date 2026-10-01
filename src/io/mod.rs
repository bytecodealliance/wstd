//! Async IO abstractions.

mod copy;
mod cursor;
mod empty;
mod read;
mod seek;

mod streams {
    mod sys {
        #[cfg(target_env = "p2")]
        pub(super) mod p2;
        #[cfg(target_env = "p3")]
        pub(super) mod p3;
    }
    #[cfg(target_env = "p2")]
    pub use sys::p2::*;
    #[cfg(target_env = "p3")]
    pub use sys::p3::*;
}

mod stdio {
    mod sys {
        #[cfg(target_env = "p2")]
        pub(super) mod p2;
        #[cfg(target_env = "p3")]
        pub(super) mod p3;
    }
    #[cfg(target_env = "p2")]
    pub use sys::p2::*;
    #[cfg(target_env = "p3")]
    pub use sys::p3::*;
}

mod write;

#[cfg(target_env = "p2")]
pub use crate::runtime::AsyncPollable;
pub use copy::*;
pub use cursor::*;
pub use empty::*;
pub use read::*;
pub use seek::*;
pub use stdio::*;
pub use streams::*;
pub use write::*;

/// The error type for I/O operations.
///
pub use std::io::Error;

/// A specialized Result type for I/O operations.
///
pub use std::io::Result;

#[cfg(test)]
mod test {
    use super::{AsyncWrite, stderr, stdout};
    use crate::runtime::block_on;

    #[test]
    // No internal predicate. Run test with --nocapture and inspect output manually.
    fn stdout_println_hello_world() {
        block_on(async {
            let mut stdout = stdout();
            let term = if stdout.is_terminal() { "is" } else { "is not" };
            stdout
                .write_all(format!("hello, world! stdout {term} a terminal\n",).as_bytes())
                .await
                .unwrap();
            stdout.flush().await.unwrap();
        })
    }

    #[test]
    // No internal predicate. Run test with --nocapture and inspect output manually.
    fn stderr_println_hello_world() {
        block_on(async {
            let mut stderr = stderr();
            let term = if stderr.is_terminal() { "is" } else { "is not" };
            stderr
                .write_all(format!("hello, world! stderr {term} a terminal\n",).as_bytes())
                .await
                .unwrap();
            stderr.flush().await.unwrap();
        })
    }
}
