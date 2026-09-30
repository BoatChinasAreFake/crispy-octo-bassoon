//! Windows builds get the game's icon (and name) embedded in minceraft.exe, so
//! it shows in Explorer and the taskbar. The icon is drawn from the game's own
//! textures: regenerate it with `minceraft --export-icon assets/minceraft.ico`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/minceraft.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/minceraft.ico");
    res.set("ProductName", "Minceraft");
    res.set("FileDescription", "Minceraft");
    if let Err(e) = res.compile() {
        // A missing resource compiler shouldn't stop the build: the game just has no icon then.
        println!("cargo:warning=Couldn't embed the icon: {e}");
    }
}
