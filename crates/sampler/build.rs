//! Builds the screen sampler (C) and links libpipewire and libsystemd (sd-bus).
fn main() {
    let out = std::process::Command::new("pkg-config")
        .args(["--cflags", "--libs", "libpipewire-0.3", "libsystemd"])
        .output()
        .expect("pkg-config is needed to find libpipewire-0.3 and libsystemd");
    let flags = String::from_utf8(out.stdout).unwrap();
    let mut build = cc::Build::new();
    build
        .file("src/sampler.c")
        .flag("-Wno-unused-parameter")
        .flag("-Wno-missing-field-initializers");
    for f in flags.split_whitespace() {
        if let Some(dir) = f.strip_prefix("-I") {
            build.include(dir);
        } else if let Some(lib) = f.strip_prefix("-l") {
            println!("cargo:rustc-link-lib={lib}");
        } else if f.starts_with("-D") {
            let (k, v) = f[2..]
                .split_once('=')
                .map_or((&f[2..], None), |(k, v)| (k, Some(v)));
            build.define(k, v);
        }
    }
    build.compile("lhubsampler");
    println!("cargo:rerun-if-changed=src/sampler.c");
}
