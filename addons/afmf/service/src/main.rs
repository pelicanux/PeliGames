use peligames_addon_kit::{manifest::Manifest, service::serve};
use std::io;

fn main() -> io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() == Some(std::ffi::OsStr::new("--peligames-launch")) {
        return peligames_afmf_service::launcher::run(args).map_err(io::Error::other);
    }
    let service = peligames_afmf_service::Service::new()?;
    let manifest: Manifest = serde_json::from_str(include_str!("../../manifest.json"))
        .expect("valid AFMF integration manifest");
    serve(
        io::stdin().lock(),
        io::stdout().lock(),
        &manifest,
        |method, params| service.handle(method, params),
    )
}
