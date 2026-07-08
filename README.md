# Hooks for the Ai Pin's base software

A system-injected set of hooks for the Humane OS and user facing apps on the Ai Pin. It redirects Humane API calls to a local server and cleans up things like telemetry and log spam.

## Current Status (v0.4.0)

PenumbraOS now provides a production-oriented native tool architecture for the Humane AI Pin.

### Features

- Dynamic native tool registry
- On-device memory
- Reverse geocoding
- Nearby search
- Weather
- Scene understanding
- Log collection
- Extensible Rust native tool host

### Native Tool Host

The native tool host is packaged directly inside the APK as an Android native library (`libpenumbra_tool_host.so`) and executed from the application's native library directory. This replaces the previous extracted executable approach and avoids execution issues associated with app-private file extraction.

The packaged host preserves controlled utility tools such as weather, reverse geocoding, nearby search, network status, and network capability inspection. General web fetch and news aggregation tools are intentionally not advertised or packaged in the APK runtime.

The repo has three main parts:

- `injector` - Selectively inject hooks into specified processes
- `hook` — The actual hooks, split out per app/functionality
- `server` — Reimplementation of cosmOS gRPC backend

## AI Pin Server Deployment

`com.penumbraos.server` is a privileged system-UID APK. Do not deploy
`server-release.apk` with plain `pm install -r -d`; that leaves the package under
`/data/app` and can make zygote abort during SELinux process-context setup.

Use the privileged system-injector staging flow documented in
[`docs/AiPinServerDeployment.md`](docs/AiPinServerDeployment.md).
