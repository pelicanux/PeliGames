use peligames_addon_kit::{manifest::Manifest, service::serve};
use peligames_optiscaler_service::Service;
use std::io;

fn main() -> io::Result<()> {
    let manifest: Manifest = serde_json::from_str(include_str!("../../manifest.json"))
        .expect("valid bundled OptiScaler manifest");
    let mut service = Service::new()?;
    serve(
        io::stdin().lock(),
        io::stdout().lock(),
        &manifest,
        |method, value| service.handle(method, value),
    )
}
