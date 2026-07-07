fn main() {
    println!("cargo::rustc-check-cfg=cfg(esp32)");
    println!("cargo::rustc-check-cfg=cfg(esp32s3)");

    embuild::espidf::sysenv::output();
}
