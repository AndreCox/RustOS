fn main() {
    // Tell Cargo to look for libraries in the /libs folder
    println!("cargo:rustc-link-search=native=./libs");

    // Tell Cargo to link the doom library (libdoom.a)
    println!("cargo:rustc-link-lib=static=doom");
}
