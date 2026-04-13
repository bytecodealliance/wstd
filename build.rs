use cfg_aliases::cfg_aliases;

fn main() {
    cfg_aliases! {
        // True when targeting a wasip3 component, either via the explicit
        // `wasip3` feature or the `wasm32-wasip3` target (`target_env = "p3"`).
        wstd_p3: { any(feature = "wasip3", target_env = "p3") },
        // True when targeting a wasip2 component, either via the `wasip2`
        // feature (default) or the `wasm32-wasip2` target. wasip3 takes
        // precedence when both apply.
        wstd_p2: { all(any(feature = "wasip2", target_env = "p2"), not(wstd_p3)) },
    }
}
