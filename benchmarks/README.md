# Fenris Benchmarks

Run the benchmark suite with:

```text
cargo bench -p benchmarks
```

The `core_layers` benchmark measures the default stack at the layer boundaries:

- Protobuf domain command encode/decode.
- Length-prefixed frame header encode/decode.
- Null, zlib, and zstd compression/decompression.
- AES-GCM encryption/decryption.
- Memory and Tokio filesystem storage chunk reads and writes.
- In-memory chunked upload/download encode, compression, encryption, decryption, and decode paths.

These benchmarks are baselines for deciding whether later work such as zstd or io_uring is justified. They should not be treated as performance claims unless run on a pinned machine profile with the same compiler and dependency versions.
