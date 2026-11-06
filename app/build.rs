fn main() {
    // ponytail: Windows-only icon/version embedding; Linux uses a .desktop file + PNG instead.
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=app.rc");
        println!("cargo:rerun-if-changed=../assets/app.ico");
        embed_resource::compile("app.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("compile app.rc");
    }
    #[cfg(not(windows))]
    println!("cargo:rerun-if-changed=build.rs");
}
