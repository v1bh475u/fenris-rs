# Fenris Architecture

Fenris is a modular Rust storage stack built around explicit layer boundaries.
The reference application is an authenticated client/server object service, and
the shared crates are structured so protocol encoding, framing, compression,
encryption, request handling, and storage can evolve independently.

## System Context

```mermaid
flowchart LR
    User["User or automation"]
    Client["client crate<br/>TUI and batch frontends"]
    Server["server crate<br/>authenticated storage server"]
    Common["common crate<br/>protocol, framing, crypto,<br/>compression, identity, storage"]
    Benchmarks["benchmarks crate<br/>Criterion layer benchmarks"]
    Storage["Storage root<br/>filesystem or in-memory backend"]

    User --> Client
    Client --> Common
    Client <-->|authenticated encrypted messages| Server
    Server --> Common
    Server --> Storage
    Benchmarks --> Common
```

The crates have distinct responsibilities:

- `common` owns the reusable contracts and default implementations.
- `client` turns user commands into typed Fenris operations and renders results.
- `server` accepts authenticated sessions and maps requests to storage actions.
- `benchmarks` measures the core layer costs without depending on a live server.

## Layered Data Path

```mermaid
flowchart TB
    Command["Client command<br/>TUI or batch"]
    Domain["FenrisCommand<br/>domain operation"]
    Protocol["ProtocolCodec<br/>Protobuf bytes"]
    Compression["Compressor<br/>null, zlib, zstd"]
    Crypto["SecureChannel<br/>AES-GCM message protection"]
    Frame["LengthPrefixedFrame<br/>bounded network frame"]
    Transport["Tokio TCP"]
    ServerConnection["Server connection"]
    Handler["RequestHandler"]
    StorageBackend["StorageBackend"]
    Backend["MemoryStorage or TokioFsStorage"]

    Command --> Domain
    Domain --> Protocol
    Protocol --> Compression
    Compression --> Crypto
    Crypto --> Frame
    Frame --> Transport
    Transport --> ServerConnection
    ServerConnection --> Handler
    Handler --> StorageBackend
    StorageBackend --> Backend
```

Every major step has a small contract:

- `ProtocolCodec` converts typed requests and responses to bytes.
- `LengthPrefixedFrame` applies explicit frame limits before transport I/O.
- `Compressor` selects the compression policy statically.
- `SecureChannel` wraps transport messages in authenticated encryption.
- `StorageBackend` exposes object and namespace behavior to the server.

## Secure Channel

Fenris uses an authenticated secure-channel handshake before application
requests are processed. The server owns a persistent Ed25519 identity key, and
the client pins the expected server public key.

```mermaid
sequenceDiagram
    participant C as Client
    participant S as Server

    C->>S: client ephemeral key
    S->>C: server ephemeral key + server identity + transcript signature
    C->>C: verify pinned server identity
    C->>C: verify transcript signature
    C->>S: handshake confirmation
    C->>C: derive session key with X25519 + HKDF-SHA256
    S->>S: derive matching session key
    C->>S: encrypted framed request
    S->>C: encrypted framed response
```

Message protection uses AES-256-GCM after the handshake. Framing, compression,
and protocol encoding sit inside the secure-channel message pipeline.

## Client Frontends

The client has two frontends over the same command execution path. TUI mode is
interactive; batch mode reads newline-delimited commands from a file or stdin
and writes either human-readable output or JSON Lines.

```mermaid
flowchart LR
    Tui["TUI mode<br/>ratatui event loop"]
    Batch["Batch mode<br/>command file or stdin"]
    RequestManager["RequestManager<br/>parse command text"]
    ConnectionManager["ConnectionManager<br/>secure channel lifecycle"]
    ResponseManager["ResponseManager<br/>format FenrisOutput"]
    Output["Screen, stdout,<br/>or JSON Lines"]

    Tui --> RequestManager
    Batch --> RequestManager
    RequestManager --> ConnectionManager
    ConnectionManager --> ResponseManager
    ResponseManager --> Tui
    ResponseManager --> Batch
    Tui --> Output
    Batch --> Output
```

This keeps command behavior consistent across interactive and automated use
cases. Commands such as `read`, `write`, `append`, and `upload` use the chunked
transfer path for object data.

## Server and Storage

The server accepts concurrent TCP sessions, authenticates the secure channel,
and dispatches typed operations to a storage backend. The request handler works
with object and namespace operations rather than directly owning filesystem I/O.

```mermaid
flowchart TB
    Listener["Server listener"]
    Semaphore["Connection limit<br/>Semaphore"]
    Connection["Connection task<br/>one per client"]
    Handler["RequestHandler"]
    Trait["StorageBackend trait"]
    Memory["MemoryStorage<br/>tests and in-memory runs"]
    TokioFs["TokioFsStorage<br/>filesystem-backed default"]
    FileOps["DefaultFileOperations<br/>path resolution and tokio::fs"]

    Listener --> Semaphore
    Semaphore --> Connection
    Connection --> Handler
    Handler --> Trait
    Trait --> Memory
    Trait --> TokioFs
    TokioFs --> FileOps
```

`StorageBackend` provides:

- object operations: put, get, chunked get, append, delete, metadata
- namespace operations: create, list, delete
- type checks: exists, is object, is namespace

`TokioFsStorage` is the default backend for the server binary. `MemoryStorage`
supports fast tests and benchmark fixtures. The object/namespace vocabulary
keeps the server logic aligned with future storage backends without forcing the
user-facing command set to change.

## Protocol and Compression

The protocol layer separates typed Fenris operations from their wire encoding.
Protobuf is the default codec, while the surrounding boundaries keep the stack
ready for future codecs.

```mermaid
flowchart LR
    FenrisCommand["FenrisCommand"]
    Request["Request protobuf"]
    Codec["ProtobufCodec"]
    Bytes["encoded bytes"]
    Compressor["Compressor"]
    Null["NullCompressor"]
    Zlib["ZlibCompressor"]
    Zstd["ZstdCompressor<br/>common/zstd feature"]

    FenrisCommand --> Request
    Request --> Codec
    Codec --> Bytes
    Bytes --> Compressor
    Compressor --> Null
    Compressor --> Zlib
    Compressor --> Zstd
```

Null compression is the default stack choice. zlib is always available, and zstd
is compiled when the `common/zstd` Cargo feature is enabled.

## Benchmark Coverage

The benchmark crate measures the main layer boundaries directly. These
measurements are the evidence gate for performance work such as specialized
storage backends.

```mermaid
flowchart TB
    Bench["core_layers benchmark"]
    ProtocolBench["Protobuf<br/>encode/decode"]
    FrameBench["Frame header<br/>encode/decode"]
    CompressionBench["Compression<br/>null, zlib, zstd"]
    CryptoBench["AES-GCM<br/>seal/open"]
    StorageBench["Storage<br/>1 MiB, 16 MiB,<br/>many small, concurrent"]
    TransferBench["Chunked transfer<br/>upload/download pipeline"]

    Bench --> ProtocolBench
    Bench --> FrameBench
    Bench --> CompressionBench
    Bench --> CryptoBench
    Bench --> StorageBench
    Bench --> TransferBench
```

CI compiles the benchmark targets with `cargo bench -p benchmarks --no-run`.
Local timing runs use `cargo bench -p benchmarks` and produce Criterion reports
under `target/`.

## Design Direction

Fenris is designed to grow by adding focused implementations behind existing
contracts:

- storage backends can move from filesystem-backed objects to richer object
  storage designs
- compression can expand through feature-gated static choices
- protocol codecs can be introduced without rewriting client or server logic
- performance work can target measured bottlenecks instead of reshaping the
  entire stack
- Linux-specific capabilities such as io_uring and eBPF can remain optional
  backend or observability work
