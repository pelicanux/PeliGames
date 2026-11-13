use peligames_addon_kit::{manifest::Manifest, service::serve};
fn main() -> std::io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() == Some(std::ffi::OsStr::new("--peligames-launch")) {
        return peligames_mako_service::launcher::run(args).map_err(std::io::Error::other);
    }
    let service = peligames_mako_service::Service::new()?;
    let manifest: Manifest =
        serde_json::from_str(include_str!("../../manifest.json")).expect("valid MAKO recipe");
    serve(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        &manifest,
        |method, params| service.handle(method, params),
    )
}
