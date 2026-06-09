//! Platform-specific backends.
//!
//! Each supported target provides an implementation under `src/sys/`, selected
//! here by a single `cfg-if`. The rest of the crate is written against the
//! backend-agnostic `crate::sys::*` re-exports below, mirroring the layout used
//! by the `polling` crate.

cfg_if::cfg_if! {
    if #[cfg(all(target_os = "wasi", target_env = "p2"))] {
        mod p2;
        use p2 as backend;
    } else {
        compile_error!("unsupported target: wstd only compiles on `wasm32-wasip2`");
    }
}

pub use backend::*;
