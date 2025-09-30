use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/mumu/external_renderer_ipc.h");

    let bindings = bindgen::Builder::default()
        .header("src/mumu/external_renderer_ipc.h")
        .ctypes_prefix("::std::os::raw")
        .clang_arg("-fms-extensions") // Handle MSVC extensions
        // Configure bindgen to generate libloading bindings
        .dynamic_library_name("test")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    // 写入到src/bindings.rs
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
