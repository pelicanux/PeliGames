//! Exercise the launcher's actual preload composition in ELF integration tests.
#[allow(dead_code)]
#[path = "../../src/preload.rs"]
mod preload;
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    let value = preload::input(
        std::path::Path::new(&args[1]),
        Some(args[2].clone()),
        Some(args[3].as_os_str()),
        args.get(4).map(|arg| arg.as_os_str()),
    )
    .unwrap();
    println!("{}", value.to_str().unwrap());
}
