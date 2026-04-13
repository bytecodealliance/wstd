cfg_if::cfg_if! {
    if #[cfg(wstd_p3)] {
        mod p3;
        use p3 as backend;
    } else if #[cfg(wstd_p2)] {
        mod p2;
        use p2 as backend;
    } else {
        compile_error!("unsupported target: wstd requires a WASI target with either the `wasip2` or `wasip3` feature");
    }
}

pub use backend::*;
