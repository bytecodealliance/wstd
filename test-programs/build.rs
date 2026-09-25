use cargo_metadata::{MetadataCommand, Package, TargetKind};
use heck::ToShoutySnakeCase;
use std::env::var_os;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(var_os("OUT_DIR").expect("OUT_DIR env var exists"));
    let nightly_toolchain = is_nightly_toolchain();

    let meta = MetadataCommand::new()
        .exec()
        .expect("cargo metadata for workspace");

    println!(
        "cargo:rerun-if-changed={}",
        meta.workspace_root.as_os_str().to_str().unwrap()
    );

    fn build_target(pkg: &str, manifest: &str, kind: &str, target: &str, out_dir: &Path) {
        // release build is required for aws sdk to not overflow wasm locals
        let status = Command::new("cargo")
            .arg("build")
            .arg(kind)
            .arg("--release")
            .arg(format!("--target={target}"))
            .arg(format!("--package={pkg}"))
            .arg(format!("--manifest-path={manifest}"))
            .env("CARGO_TARGET_DIR", out_dir)
            .env("CARGO_PROFILE_DEV_DEBUG", "2")
            .env("RUSTFLAGS", rustflags())
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .status()
            .expect("cargo build wstd examples");
        assert!(status.success());
    }

    fn build_targets(
        pkg: &str,
        manifest: &str,
        kind: &str,
        nightly_toolchain: bool,
        out_dir: &Path,
    ) {
        build_target(pkg, manifest, kind, "wasm32-wasip2", out_dir);

        if nightly_toolchain {
            build_target(pkg, manifest, kind, "wasm32-wasip3", out_dir);
        }
    }

    build_targets(
        "wstd",
        "../Cargo.toml",
        "--examples",
        nightly_toolchain,
        &out_dir,
    );
    build_targets(
        "wstd-axum",
        "../Cargo.toml",
        "--examples",
        nightly_toolchain,
        &out_dir,
    );
    build_targets(
        "wstd-aws-example",
        "../aws-example/Cargo.toml",
        "--bins",
        // TODO: enable when aws example is running for WASIp3
        false,
        &out_dir,
    );

    let mut generated_code = "// THIS FILE IS GENERATED CODE\n".to_string();
    generated_code += &format!("pub const NIGHTLY_TOOLCHAIN: bool = {nightly_toolchain};\n\n");

    fn module_for(name: &str, kind: TargetKind, out_dir: &Path, meta: &Package) -> String {
        let mut generated_code = String::new();
        for target in meta.targets.iter() {
            generated_code += &format!("// {target:?} \n");
        }
        generated_code += &format!("pub mod {name} {{");
        for binary in meta.targets.iter().filter(|t| t.kind == [kind.clone()]) {
            let mut component_path = out_dir.join("wasm32-wasip2").join("release");
            let mut p3_component_path = out_dir.join("wasm32-wasip3").join("release");
            match kind {
                TargetKind::Bin => {}
                TargetKind::Example => {
                    component_path = component_path.join("examples");
                    p3_component_path = p3_component_path.join("examples");
                }
                _ => unimplemented!("path interpolation for TargetKind {kind:?}"),
            }
            component_path = component_path.join(format!("{}.wasm", binary.name));
            p3_component_path = p3_component_path.join(format!("{}.wasm", binary.name));

            let const_name = binary.name.to_shouty_snake_case();
            generated_code += &format!(
                "pub const {const_name}: &str = {:?};\n",
                component_path.as_os_str().to_str().expect("path is str")
            );
            generated_code += &format!(
                "pub const {const_name}_P3: &str = {:?};\n",
                p3_component_path.as_os_str().to_str().expect("path is str")
            );
        }
        generated_code += "}\n\n"; // end `pub mod {name}`
        generated_code
    }

    generated_code += &module_for(
        "_wstd",
        TargetKind::Example,
        &out_dir,
        meta.packages
            .iter()
            .find(|p| *p.name == "wstd")
            .expect("wstd is in cargo metadata"),
    );
    generated_code += "pub use _wstd::*;\n\n";
    generated_code += &module_for(
        "axum",
        TargetKind::Example,
        &out_dir,
        meta.packages
            .iter()
            .find(|p| *p.name == "wstd-axum")
            .expect("wstd-axum is in cargo metadata"),
    );
    let aws_example_meta = MetadataCommand::new()
        .manifest_path("../aws-example/Cargo.toml")
        .exec()
        .expect("cargo metadata for aws-example");
    generated_code += &module_for(
        "aws",
        TargetKind::Bin,
        &out_dir,
        aws_example_meta
            .packages
            .iter()
            .find(|p| *p.name == "wstd-aws-example")
            .expect("wstd-aws-example is in cargo metadata"),
    );

    std::fs::write(out_dir.join("gen.rs"), generated_code).unwrap();
}

fn is_nightly_toolchain() -> bool {
    let rustc = var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc)
        .arg("--version")
        .output()
        .expect("query active rustc version");
    assert!(
        output.status.success(),
        "failed to query active rustc version"
    );
    String::from_utf8(output.stdout)
        .expect("rustc version is UTF-8")
        .contains("-nightly")
}

fn rustflags() -> &'static str {
    match option_env!("RUSTFLAGS") {
        Some(s) if s.contains("-D warnings") => "-D warnings",
        _ => "",
    }
}
