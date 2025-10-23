use cargo_metadata::TargetKind;
use heck::ToShoutySnakeCase;
use std::env::var_os;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(var_os("OUT_DIR").expect("OUT_DIR env var exists"));

    let meta = cargo_metadata::MetadataCommand::new()
        .exec()
        .expect("cargo metadata");
    let wstd_meta = meta
        .packages
        .iter()
        .find(|p| *p.name == "wstd")
        .expect("wstd is in cargo metadata");
    let wstd_axum_meta = meta
        .packages
        .iter()
        .find(|p| *p.name == "wstd-axum")
        .expect("wstd is in cargo metadata");

    let wstd_root = wstd_meta.manifest_path.parent().unwrap();
    println!(
        "cargo:rerun-if-changed={}",
        wstd_root.as_os_str().to_str().unwrap()
    );

    fn build_examples(pkg: &str, out_dir: &PathBuf) {
        let status = Command::new("cargo")
            .arg("build")
            .arg("--examples")
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

    let mut generated_code = "// THIS FILE IS GENERATED CODE\n".to_string();

    for binary in wstd_meta
        .targets
        .iter()
        .filter(|t| t.kind == [TargetKind::Example])
    {
        let component_path = out_dir
            .join("wasm32-wasip2")
            .join("debug")
            .join("examples")
            .join(format!("{}.wasm", binary.name));

        let const_name = binary.name.to_shouty_snake_case();
        generated_code += &format!(
            "pub const {const_name}: &str = {:?};\n",
            component_path.as_os_str().to_str().expect("path is str")
        );
    }

    generated_code += "pub mod axum {";
    for binary in wstd_axum_meta
        .targets
        .iter()
        .filter(|t| t.kind == [TargetKind::Example])
    {
        let component_path = out_dir
            .join("wasm32-wasip2")
            .join("debug")
            .join("examples")
            .join(format!("{}.wasm", binary.name));

        let const_name = binary.name.to_shouty_snake_case();
        generated_code += &format!(
            "pub const {const_name}: &str = {:?};\n",
            component_path.as_os_str().to_str().expect("path is str")
        );
    }
    generated_code += "}"; // end `pub mod axum`

    std::fs::write(out_dir.join("gen.rs"), generated_code).unwrap();
}

fn rustflags() -> &'static str {
    match option_env!("RUSTFLAGS") {
        Some(s) if s.contains("-D warnings") => "-D warnings",
        _ => "",
    }
}
