//! Service-side protocol loop. Feature handlers never write protocol envelopes.
use crate::{
    manifest::{Manifest, HOST_API},
    protocol::{Initialize, Request, Response, RpcError, MAX_MESSAGE_BYTES},
};
use serde_json::Value;
use std::io::{self, BufRead, Read, Write};

/// Bounded stdio server with a required handshake and automatic shutdown/EOF.
pub fn serve(
    mut input: impl BufRead,
    mut output: impl Write,
    manifest: &Manifest,
    mut handle: impl FnMut(&str, Value) -> Result<Value, RpcError>,
) -> io::Result<()> {
    let mut initialized = false;
    loop {
        let mut bytes = Vec::new();
        let read = input
            .by_ref()
            .take((MAX_MESSAGE_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)?;
        if read == 0 {
            return Ok(());
        }
        if read > MAX_MESSAGE_BYTES {
            return Err(io::Error::other("Protocol message too large"));
        }
        let line = std::str::from_utf8(&bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let request = Request::parse(line).map_err(io::Error::other)?;
        let shutdown = initialized && request.method == "shutdown";
        let response = match request.method.as_str() {
            "initialize"
                if request.params.get("host_api").and_then(Value::as_u64)
                    == Some(HOST_API.into()) =>
            {
                initialized = true;
                Response::success(
                    request.id,
                    serde_json::to_value(Initialize {
                        host_api: HOST_API,
                        addon_id: manifest.id.clone(),
                        addon_version: manifest.version.clone(),
                    })?,
                )
            }
            "initialize" => Response::failure(request.id, -32602, "Unsupported host API"),
            _ if !initialized => {
                Response::failure(request.id, -32000, "Initialize the service first")
            }
            "shutdown" => Response::success(request.id, Value::Null),
            method => match handle(method, request.params) {
                Ok(result) => Response::success(request.id, result),
                Err(error) => Response::failure(request.id, error.code, error.message),
            },
        };
        let mut encoded = serde_json::to_vec(&response)?;
        if encoded.len() + 1 > MAX_MESSAGE_BYTES {
            encoded = serde_json::to_vec(&Response::failure(
                request.id,
                -32001,
                "Service result is too large",
            ))?;
        }
        output.write_all(&encoded)?;
        output.write_all(b"\n")?;
        output.flush()?;
        if shutdown {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    fn manifest() -> Manifest {
        serde_json::from_str(include_str!("../../template/manifest.json")).unwrap()
    }
    #[test]
    fn handshake_errors_and_shutdown_are_owned_by_the_protocol_loop() {
        let requests = [
            Request::new(1, "echo", Value::Null),
            Request::new(2, "initialize", serde_json::json!({"host_api":0})),
            Request::new(3, "initialize", serde_json::json!({"host_api":HOST_API})),
            Request::new(4, "unknown", Value::Null),
            Request::new(5, "echo", serde_json::json!({"value":42})),
            Request::new(6, "shutdown", Value::Null),
            Request::new(7, "echo", Value::Null),
        ];
        let input = requests
            .iter()
            .map(|r| serde_json::to_string(r).unwrap() + "\n")
            .collect::<String>();
        let mut output = Vec::new();
        let mut calls = Vec::new();
        serve(
            Cursor::new(input),
            &mut output,
            &manifest(),
            |method, params| {
                calls.push(method.to_string());
                if method == "echo" {
                    Ok(params)
                } else {
                    Err(RpcError {
                        code: -32601,
                        message: "Method not found".into(),
                    })
                }
            },
        )
        .unwrap();
        let lines = std::str::from_utf8(&output)
            .unwrap()
            .lines()
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 6);
        assert!(Response::parse(lines[0], 1).is_err());
        assert!(Response::parse(lines[1], 2).is_err());
        let identity: Initialize =
            serde_json::from_value(Response::parse(lines[2], 3).unwrap()).unwrap();
        identity.validate("example-service", "0.1.0").unwrap();
        assert!(Response::parse(lines[3], 4).is_err());
        assert_eq!(
            Response::parse(lines[4], 5).unwrap(),
            serde_json::json!({"value":42})
        );
        assert_eq!(Response::parse(lines[5], 6).unwrap(), Value::Null);
        assert_eq!(calls, ["unknown", "echo"]);
    }
    #[test]
    fn bounded_input_and_output_and_eof() {
        let mut output = Vec::new();
        assert!(serve(
            Cursor::new(vec![b'x'; MAX_MESSAGE_BYTES + 1]),
            &mut output,
            &manifest(),
            |_, _| unreachable!()
        )
        .is_err());
        assert!(output.is_empty());
        let input = [
            Request::new(1, "initialize", serde_json::json!({"host_api":1})),
            Request::new(2, "large", Value::Null),
        ]
        .iter()
        .map(|r| serde_json::to_string(r).unwrap() + "\n")
        .collect::<String>();
        serve(Cursor::new(input), &mut output, &manifest(), |_, _| {
            Ok(Value::String("x".repeat(MAX_MESSAGE_BYTES)))
        })
        .unwrap();
        let text = std::str::from_utf8(&output).unwrap();
        assert!(Response::parse(text.lines().nth(1).unwrap(), 2).is_err());
        assert!(output.len() < 1024);
        serve(
            Cursor::new(""),
            &mut output,
            &manifest(),
            |_, _| unreachable!(),
        )
        .unwrap();
    }
}
