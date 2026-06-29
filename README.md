# Fenris

[![CI](https://github.com/v1bh475u/fenris-rs/workflows/CI/badge.svg)](https://github.com/v1bh475u/fenris-rs/actions/workflows/ci.yml)
[![Coverage](https://codecov.io/gh/v1bh475u/fenris-rs/branch/master/graph/badge.svg)](https://codecov.io/gh/v1bh475u/fenris-rs)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Fast Encrypted Networked Robust Information Storage.

Fenris is a modular Rust framework for encrypted, networked storage systems. The
current repository contains a complete client/server reference implementation
with authenticated transport, bounded framing, chunked object transfer,
swappable storage backends, optional compression, and a benchmark suite for the
core data path.

## Highlights

- Encrypted client/server transport built on X25519, HKDF-SHA256, and
  AES-256-GCM.
- Authenticated server identity with Ed25519 keys and client-side identity
  pinning.
- Bounded length-prefixed frames to keep network message sizes explicit.
- Protocol codec boundary with Protobuf as the default wire representation.
- Statically selectable compression with null compression by default, zlib
  support, and optional zstd support.
- Chunked object reads, writes, appends, and uploads for large payloads.
- Storage backend abstraction with memory-backed and Tokio filesystem
  implementations.
- Terminal client with both interactive TUI mode and batch execution mode.
- Criterion benchmarks for protocol, framing, compression, crypto, storage, and
  chunked transfer paths.

## Workspace

```text
common      Shared protocol, crypto, framing, compression, identity, and storage layers
client      TUI and batch clients over the shared command execution path
server      Concurrent authenticated storage server
benchmarks  Criterion benchmarks for the core Fenris data path
```

The default runtime stack is intentionally readable:

```text
Client command
  -> Protobuf protocol codec
  -> bounded frame codec
  -> compression policy
  -> authenticated encrypted channel
  -> server request handler
  -> storage backend
```

## Quickstart

Build the workspace:

```sh
cargo build --workspace
```

Create a storage directory and run the server. The server identity key is loaded
from the given path, or generated there on first start.

```sh
mkdir -p /tmp/fenris-data
cargo run -p server -- \
  --base-dir /tmp/fenris-data \
  --identity-key /tmp/fenris-server.key
```

The server prints its public identity as a hex string:

```text
Server identity: <server-identity-hex>
```

Use that value to pin the expected server identity from the client.

Interactive TUI mode:

```sh
cargo run -p client -- --server-identity <server-identity-hex> tui
```

Batch mode from a command file:

```sh
cat > /tmp/fenris-commands.txt <<'EOF'
ping
mkdir docs
write docs/hello.txt hello from fenris
read docs/hello.txt
info docs/hello.txt
ls docs
EOF

cargo run -p client -- \
  --server-identity <server-identity-hex> \
  batch \
  --address 127.0.0.1 \
  --port 5555 \
  --commands-file /tmp/fenris-commands.txt \
  --output human
```

Batch mode also supports JSON Lines output:

```sh
cargo run -p client -- \
  --server-identity <server-identity-hex> \
  batch \
  --commands-file /tmp/fenris-commands.txt \
  --output jsonl
```

## Client Commands

The TUI and batch clients share the same command parser and request execution
path.

```text
ping                         Check server connectivity
ls [path]                    List a namespace
cd [path]                    Change the current namespace
read <path>                  Read an object
write <path> <data>          Replace an object with inline data
append <path> <data>         Append inline data to an object
upload <local> <remote>      Upload a local file as a remote object
create <path>                Create an empty object
rm <path>                    Delete an object
mkdir <path>                 Create a namespace
rmdir <path>                 Delete a namespace
info <path>                  Show object or namespace metadata
```

## Architecture

Fenris is organized around small contracts that can be tested and replaced
independently:

- `ProtocolCodec` converts typed Fenris commands and outputs to bytes.
- `LengthPrefixedFrame` bounds message exchange at the transport boundary.
- `Compressor` selects null, zlib, or zstd compression at compile time.
- `SecureChannel` handles handshake, encryption, authentication, framing, and
  message protection.
- `StorageBackend` exposes object and namespace operations independent of the
  concrete backend.

See [docs/Architecture.md](docs/Architecture.md) for diagrams covering module
dependencies, client/server data flow, secure-channel setup, storage backends,
and benchmark coverage.

## Benchmarks

Fenris includes a dedicated benchmark crate for the core layers.

Compile benchmark targets in CI or before a pull request:

```sh
cargo bench -p benchmarks --no-run
```

Run local timing benchmarks:

```sh
cargo bench -p benchmarks
```

The benchmark suite covers Protobuf encode/decode, frame header handling,
null/zlib/zstd compression, AES-GCM encryption, memory and filesystem storage,
storage stress shapes, and in-memory chunked transfer paths.

## Roadmap

Fenris is moving toward a richer object-storage-oriented framework while keeping
the reference client/server implementation practical and easy to inspect.

Near-term work:

- Expand client and server coverage around command execution and request
  handling.
- Continue shaping filesystem-style commands around object and namespace
  semantics.
- Use the benchmark suite to guide optimization work.
- Explore Linux-specific storage backends such as io_uring when measurements
  justify the added complexity.
- Add observability-focused tooling, with eBPF as a future diagnostics path.
