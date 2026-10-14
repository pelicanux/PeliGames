# MatrixRTC device keys

`matrix-js-sdk@44.0.0.patch` loads and tracks recipients' device keys before Olm to-device encryption. MatrixRTC can run in rooms whose text messages are not encrypted; those rooms do not create a room encryptor to track the participants automatically. Without this step, the SDK silently skips unknown devices and a participant may never receive the media key.

The change uses the SDK's existing Rust crypto engine and outgoing request manager. It does not change device signatures, trust policy, cipher choice or key exchange format. Both the published JavaScript and its TypeScript source carry the same correction. Matrix JS SDK is Apache-2.0.

Regression test: `bun test src/services/matrixRtc.integration.test.ts`. Two independent official SDK crypto engines join a simulated Matrix room, exchange Olm-encrypted media keys, and rotate the key after a participant leaves. This is a protocol fixture, not a substitute for a call between real accounts on the deployed service.
