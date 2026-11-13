use peligames_addon_kit::{
    manifest::Manifest,
    protocol::{RpcError, ServiceCall},
    service::serve,
};
use std::io;

fn main() -> io::Result<()> {
    let manifest: Manifest = serde_json::from_str(include_str!("../../manifest.json"))
        .expect("valid bundled example manifest");
    serve(
        io::stdin().lock(),
        io::stdout().lock(),
        &manifest,
        |method, params| match method {
            "echo" => serde_json::from_value::<ServiceCall<serde_json::Value>>(params)
                .map(|call| call.request)
                .map_err(|_| RpcError {
                    code: -32602,
                    message: "Invalid service call".into(),
                }),
            _ => Err(RpcError {
                code: -32601,
                message: "Method not found".into(),
            }),
        },
    )
}
