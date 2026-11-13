use peligames_addon_kit::{manifest::Manifest, service::serve};
fn main() -> std::io::Result<()> {
    let manifest: Manifest =
        serde_json::from_str(include_str!("../../manifest.json")).expect("valid DLSSNR recipe");
    let service = peligames_dlssnr_service::Service::new()?;
    serve(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        &manifest,
        |method, params| service.handle(method, params),
    )
}
