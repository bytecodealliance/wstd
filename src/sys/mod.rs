cfg_if::cfg_if! {
    if #[cfg(any(feature = "wasip3", all(target_os = "wasi", target_env = "p3")))] {
        mod p3;
        use p3 as backend;
    } else {
        mod p2;
        use p2 as backend;
    }
}

pub use backend::*;
