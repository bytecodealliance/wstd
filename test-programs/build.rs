use cargo_metadata::{MetadataCommand, Package, TargetKind};
use heck::ToShoutySnakeCase;
use std::env::var_os;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(var_os("OUT_DIR").expect("OUT_DIR env var exists"));

    let meta = MetadataCommand::new().exec().expect("cargo metadata");

    println!(
        "cargo:rerun-if-changed={}",
        meta.workspace_root.as_os_str().to_str().unwrap()
    );

    fn build_examples(pkg: &str, out_dir: &PathBuf) {
        // release build is required for aws sdk to not overflow wasm locals
        let status = Command::new("cargo")
            .arg("build")
            .arg("--examples")
            .arg("--release")
            .arg("--target=wasm32-wasip2")
            .arg(format!("--package={pkg}"))
            .env("CARGO_TARGET_DIR", out_dir)
            .env("CARGO_PROFILE_DEV_DEBUG", "2")
            .env("RUSTFLAGS", rustflags())
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .status()
            .expect("cargo build wstd examples");
        assert!(status.success());
    }
    build_examples("wstd", &out_dir);
    build_examples("wstd-axum", &out_dir);
    build_examples("wstd-aws", &out_dir);

    let mut generated_code = "// THIS FILE IS GENERATED CODE\n".to_string();

    fn module_for(name: &str, out_dir: &Path, meta: &Package) -> String {
        let mut generated_code = String::new();
        generated_code += &format!("pub mod {name} {{");
        for binary in meta
            .targets
            .iter()
            .filter(|t| t.kind == [TargetKind::Example])
        {
            let component_path = out_dir
                .join("wasm32-wasip2")
                .join("release")
                .join("examples")
                .join(format!("{}.wasm", binary.name));

            let const_name = binary.name.to_shouty_snake_case();
            generated_code += &format!(
                "pub const {const_name}: &str = {:?};\n",
                component_path.as_os_str().to_str().expect("path is str")
            );
        }
        generated_code += "}\n\n"; // end `pub mod {name}`
        generated_code
    }

    generated_code += &module_for(
        "_wstd",
        &out_dir,
        meta.packages
            .iter()
            .find(|p| *p.name == "wstd")
            .expect("wstd is in cargo metadata"),
    );
    generated_code += "pub use _wstd::*;\n\n";
    generated_code += &module_for(
        "axum",
        &out_dir,
        meta.packages
            .iter()
            .find(|p| *p.name == "wstd-axum")
            .expect("wstd-axum is in cargo metadata"),
    );
    generated_code += &module_for(
        "aws",
        &out_dir,
        meta.packages
            .iter()
            .find(|p| *p.name == "wstd-aws")
            .expect("wstd-aws is in cargo metadata"),
    );

    std::fs::write(out_dir.join("gen.rs"), generated_code).unwrap();
}

fn rustflags() -> &'static str {
    match option_env!("RUSTFLAGS") {
        Some(s) if s.contains("-D warnings") => "-D warnings",
        _ => "",
    }
}
