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
- News aggregation
- Scene understanding
- Log collection
- Extensible Rust native tool host

### Native Tool Host

The native tool host is packaged directly inside the APK as an Android native library (`libpenumbra_tool_host.so`) and executed from the application's native library directory. This replaces the previous extracted executable approach and avoids execution issues associated with app-private file extraction.

### Current News Providers

- BBC
- NPR
- Hacker News
- Latent Space

Additional providers can be added by implementing a new provider in the native tool host and registering it with the dynamic tool registry.

The repo has three main parts:

- `injector` - Selectively inject hooks into specified processes
- `hook` — The actual hooks, split out per app/functionality
- `server` — Reimplementation of cosmOS gRPC backend
