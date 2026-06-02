fn main() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap())
        .join("locale/translators.rs");
    include_po::generate_locales_from_dir("locales", out).unwrap();

    slint_build::compile("ui/main.slint").unwrap();
}
