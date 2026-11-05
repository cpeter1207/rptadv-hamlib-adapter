//! Generate private bindings and symbolic model names from installed Hamlib headers.
use std::{env, fmt::Write, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rustc-link-lib=dylib=hamlib");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=wrapper.h");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let includedir = Command::new("pkg-config")
        .args(["--variable=includedir", "hamlib"])
        .output()
        .expect("find installed Hamlib headers");
    assert!(
        includedir.status.success(),
        "Hamlib development package is required"
    );
    let includedir = String::from_utf8(includedir.stdout).expect("UTF-8 include path");
    let model_header = PathBuf::from(includedir.trim()).join("hamlib/riglist.h");
    println!("cargo:rerun-if-changed={}", model_header.display());
    let header = fs::read_to_string(model_header).expect("read installed Hamlib model definitions");
    let mut names: Vec<_> = header
        .lines()
        .filter_map(|line| {
            let define = line
                .trim_start()
                .strip_prefix('#')?
                .trim_start()
                .strip_prefix("define")?;
            let name = define.split_whitespace().next()?;
            (name.starts_with("RIG_MODEL_") && !name.contains('(')).then_some(name)
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    assert!(
        !names.is_empty(),
        "installed headers did not define model names"
    );
    // libclang evaluates function-like macro expansions through ordinary C
    // enum initializers. Bindgen's macro parser alone omits these models.
    let mut aliases = String::new();
    for name in &names {
        writeln!(aliases, "RPTADV_{name} = {name},").expect("write alias text");
    }
    let aliases = format!("enum {{ RPTADV_VFO_CURRENT = RIG_VFO_CURR,\n{aliases}}};\n");
    let alias_path = output.join("rptadv_models.h");
    if fs::read_to_string(&alias_path).ok().as_deref() != Some(&aliases) {
        fs::write(&alias_path, aliases).expect("write evaluated model definitions");
    }
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", output.display()))
        .allowlist_function("rig_.*")
        .allowlist_var("RIG_.*")
        .allowlist_var("RPTADV_.*")
        .allowlist_type("rig_caps")
        .allowlist_type("rig_state")
        .opaque_type("s_rig")
        .prepend_enum_name(false)
        .layout_tests(false)
        .generate_comments(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("generate installed Hamlib bindings");
    let text = bindings.to_string();
    fs::write(output.join("hamlib.rs"), &text).expect("write Hamlib bindings");
    let mut entries = String::new();
    for name in &names {
        writeln!(entries, "(\"{name}\", raw::RPTADV_{name}),").expect("write model text");
    }
    fs::write(
        output.join("models.rs"),
        format!("const MODELS: &[(&str, u32)] = &[\n{entries}];\n"),
    )
    .expect("write symbolic Hamlib model catalog");
}
