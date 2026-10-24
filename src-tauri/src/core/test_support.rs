//! Helpers shared by tests only.

/// Tests write small scripts and run them while other tests fork in parallel.
/// A fork that has not reached exec yet briefly holds the new script's write
/// descriptor, and running the script then fails with ETXTBSY: retry it.
pub(crate) trait RetryOutput {
    fn output_retrying(&mut self) -> std::process::Output;
}
impl RetryOutput for std::process::Command {
    fn output_retrying(&mut self) -> std::process::Output {
        for _ in 0..100 {
            match self.output() {
                Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => std::thread::sleep(std::time::Duration::from_millis(10)),
                result => return result.unwrap(),
            }
        }
        panic!("o script de teste continuou ocupado (ETXTBSY)");
    }
}
