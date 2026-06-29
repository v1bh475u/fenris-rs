use benchmarks::{
    CHUNK_PAYLOAD_SIZE, CONCURRENT_OBJECT_COUNT, CONCURRENT_OBJECT_SIZE, LARGE_STORAGE_OBJECT_SIZE,
    LARGE_TRANSFER_SIZE, MANY_SMALL_OBJECT_COUNT, SMALL_PAYLOAD_SIZE, compressible_payload,
    concurrent_object_paths, deterministic_payload, many_small_object_paths,
    put_concurrent_objects, read_all_chunks, read_concurrent_objects, read_objects,
    sample_content_output, sample_write_command, seed_many_small_objects, seeded_memory_storage,
};
use common::{
    CompressionManager, CryptoManager, DEFAULT_TRANSFER_CHUNK_SIZE, FenrisCommand, FenrisOutput,
    FrameLimits, IV_SIZE, KEY_SIZE, LengthPrefixedFrame, MemoryStorage, ProtobufCodec,
    ProtocolCodec, StorageBackend, TokioFsStorage, TransferChunk, ZlibCompressor, ZstdCompressor,
    compression::NullCompressor,
    crypto::{AesGcmEncryptor, HkdfSha256Deriver, X25519KeyExchanger},
};
use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use std::path::Path;

const BENCH_SIZES: [usize; 3] = [SMALL_PAYLOAD_SIZE, CHUNK_PAYLOAD_SIZE, LARGE_TRANSFER_SIZE];

fn bench_protocol_codec(c: &mut Criterion) {
    let mut group = c.benchmark_group("protocol_codec");

    for size in BENCH_SIZES {
        group.throughput(Throughput::Bytes(size as u64));
        let command = sample_write_command(size);
        let encoded = <ProtobufCodec as ProtocolCodec<FenrisCommand>>::encode(&command).unwrap();

        group.bench_with_input(
            BenchmarkId::new("encode_command", size),
            &command,
            |b, command| {
                b.iter(|| {
                    black_box(
                        <ProtobufCodec as ProtocolCodec<FenrisCommand>>::encode(black_box(command))
                            .unwrap(),
                    )
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("decode_command", size),
            &encoded,
            |b, encoded| {
                b.iter(|| {
                    let decoded: FenrisCommand =
                        <ProtobufCodec as ProtocolCodec<FenrisCommand>>::decode(black_box(encoded))
                            .unwrap();
                    black_box(decoded)
                })
            },
        );
    }

    group.finish();
}

fn bench_frame_codec(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_codec");
    let limits = FrameLimits {
        max_frame_size: LARGE_TRANSFER_SIZE,
    };

    for size in BENCH_SIZES {
        let header = LengthPrefixedFrame::encode_len(size).unwrap();

        group.bench_with_input(BenchmarkId::new("encode_len", size), &size, |b, size| {
            b.iter(|| black_box(LengthPrefixedFrame::encode_len(black_box(*size)).unwrap()))
        });

        group.bench_with_input(
            BenchmarkId::new("decode_len", size),
            &header,
            |b, header| {
                b.iter(|| {
                    black_box(LengthPrefixedFrame::decode_len(black_box(*header), limits).unwrap())
                })
            },
        );
    }

    group.finish();
}

fn bench_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");

    for size in [SMALL_PAYLOAD_SIZE, CHUNK_PAYLOAD_SIZE] {
        group.throughput(Throughput::Bytes(size as u64));
        let payload = compressible_payload(size);
        let null = CompressionManager::new(NullCompressor);
        let zlib = CompressionManager::new(ZlibCompressor::default());
        let zstd = CompressionManager::new(ZstdCompressor::default());
        let null_compressed = null.compress(&payload).unwrap();
        let zlib_compressed = zlib.compress(&payload).unwrap();
        let zstd_compressed = zstd.compress(&payload).unwrap();

        group.bench_with_input(
            BenchmarkId::new("null_compress", size),
            &payload,
            |b, payload| b.iter(|| black_box(null.compress(black_box(payload)).unwrap())),
        );

        group.bench_with_input(
            BenchmarkId::new("null_decompress", size),
            &null_compressed,
            |b, compressed| b.iter(|| black_box(null.decompress(black_box(compressed)).unwrap())),
        );

        group.bench_with_input(
            BenchmarkId::new("zlib_compress", size),
            &payload,
            |b, payload| b.iter(|| black_box(zlib.compress(black_box(payload)).unwrap())),
        );

        group.bench_with_input(
            BenchmarkId::new("zlib_decompress", size),
            &zlib_compressed,
            |b, compressed| b.iter(|| black_box(zlib.decompress(black_box(compressed)).unwrap())),
        );

        group.bench_with_input(
            BenchmarkId::new("zstd_compress", size),
            &payload,
            |b, payload| b.iter(|| black_box(zstd.compress(black_box(payload)).unwrap())),
        );

        group.bench_with_input(
            BenchmarkId::new("zstd_decompress", size),
            &zstd_compressed,
            |b, compressed| b.iter(|| black_box(zstd.decompress(black_box(compressed)).unwrap())),
        );
    }

    group.finish();
}

fn bench_crypto(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto");
    let crypto = CryptoManager::new(
        AesGcmEncryptor,
        X25519KeyExchanger,
        HkdfSha256Deriver::default(),
    );
    let key = [7; KEY_SIZE];
    let iv = [3; IV_SIZE];

    for size in BENCH_SIZES {
        group.throughput(Throughput::Bytes(size as u64));
        let payload = deterministic_payload(size);
        let ciphertext = crypto.encrypt(&payload, &key, &iv).unwrap();

        group.bench_with_input(
            BenchmarkId::new("aes_gcm_encrypt", size),
            &payload,
            |b, payload| {
                b.iter(|| black_box(crypto.encrypt(black_box(payload), &key, &iv).unwrap()))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("aes_gcm_decrypt", size),
            &ciphertext,
            |b, ciphertext| {
                b.iter(|| black_box(crypto.decrypt(black_box(ciphertext), &key, &iv).unwrap()))
            },
        );
    }

    group.finish();
}

fn bench_storage(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("storage");
    group.throughput(Throughput::Bytes(LARGE_TRANSFER_SIZE as u64));

    let payload = deterministic_payload(LARGE_TRANSFER_SIZE);

    group.bench_function("memory_put_1_mib", |b| {
        b.to_async(&runtime).iter_batched(
            || (MemoryStorage::new(), payload.clone()),
            |(storage, payload)| async move {
                storage
                    .put_object(Path::new("/bench.bin"), &payload)
                    .await
                    .unwrap();
            },
            BatchSize::SmallInput,
        )
    });

    let memory_storage = runtime
        .block_on(seeded_memory_storage(LARGE_TRANSFER_SIZE))
        .unwrap();
    group.bench_function("memory_chunked_read_1_mib", |b| {
        b.to_async(&runtime).iter(|| {
            let storage = memory_storage.clone();
            async move {
                let total = read_all_chunks(
                    &storage,
                    Path::new("/bench.bin"),
                    DEFAULT_TRANSFER_CHUNK_SIZE,
                )
                .await
                .unwrap();
                black_box(total);
            }
        })
    });

    let large_payload = deterministic_payload(LARGE_STORAGE_OBJECT_SIZE);

    group.throughput(Throughput::Bytes(LARGE_STORAGE_OBJECT_SIZE as u64));
    group.bench_function("memory_put_16_mib", |b| {
        b.to_async(&runtime).iter_batched(
            || (MemoryStorage::new(), large_payload.clone()),
            |(storage, payload)| async move {
                storage
                    .put_object(Path::new("/bench-large.bin"), &payload)
                    .await
                    .unwrap();
            },
            BatchSize::LargeInput,
        )
    });

    let large_memory_storage = runtime
        .block_on(seeded_memory_storage(LARGE_STORAGE_OBJECT_SIZE))
        .unwrap();
    group.bench_function("memory_chunked_read_16_mib", |b| {
        b.to_async(&runtime).iter(|| {
            let storage = large_memory_storage.clone();
            async move {
                let total = read_all_chunks(
                    &storage,
                    Path::new("/bench.bin"),
                    DEFAULT_TRANSFER_CHUNK_SIZE,
                )
                .await
                .unwrap();
                black_box(total);
            }
        })
    });

    group.throughput(Throughput::Bytes(
        (MANY_SMALL_OBJECT_COUNT * SMALL_PAYLOAD_SIZE) as u64,
    ));
    group.bench_function("memory_many_small_put_256x4_kib", |b| {
        b.to_async(&runtime).iter_batched(
            MemoryStorage::new,
            |storage| async move {
                black_box(seed_many_small_objects(&storage).await.unwrap());
            },
            BatchSize::SmallInput,
        )
    });

    let many_small_memory_storage = MemoryStorage::new();
    runtime
        .block_on(seed_many_small_objects(&many_small_memory_storage))
        .unwrap();
    let many_small_paths = many_small_object_paths();
    group.bench_function("memory_many_small_read_256x4_kib", |b| {
        b.to_async(&runtime).iter(|| {
            let storage = many_small_memory_storage.clone();
            let paths = many_small_paths.clone();
            async move {
                black_box(read_objects(&storage, &paths).await.unwrap());
            }
        })
    });

    group.throughput(Throughput::Bytes(
        (CONCURRENT_OBJECT_COUNT * CONCURRENT_OBJECT_SIZE) as u64,
    ));
    group.bench_function("memory_concurrent_put_8x1_mib", |b| {
        b.to_async(&runtime).iter_batched(
            MemoryStorage::new,
            |storage| async move {
                black_box(
                    put_concurrent_objects(
                        storage,
                        CONCURRENT_OBJECT_COUNT,
                        CONCURRENT_OBJECT_SIZE,
                    )
                    .await
                    .unwrap(),
                );
            },
            BatchSize::SmallInput,
        )
    });

    let concurrent_memory_storage = MemoryStorage::new();
    runtime
        .block_on(put_concurrent_objects(
            concurrent_memory_storage.clone(),
            CONCURRENT_OBJECT_COUNT,
            CONCURRENT_OBJECT_SIZE,
        ))
        .unwrap();
    group.bench_function("memory_concurrent_read_8x1_mib", |b| {
        b.to_async(&runtime).iter(|| {
            let storage = concurrent_memory_storage.clone();
            async move {
                black_box(
                    read_concurrent_objects(storage, concurrent_object_paths())
                        .await
                        .unwrap(),
                );
            }
        })
    });

    let temp_dir = tempfile::tempdir().unwrap();
    let fs_storage = TokioFsStorage::new(temp_dir.path().to_path_buf());
    runtime
        .block_on(fs_storage.put_object(Path::new("bench.bin"), &payload))
        .unwrap();

    group.throughput(Throughput::Bytes(LARGE_TRANSFER_SIZE as u64));
    group.bench_function("tokio_fs_put_1_mib", |b| {
        b.to_async(&runtime).iter_batched(
            || payload.clone(),
            |payload| {
                let storage = fs_storage.clone();
                async move {
                    storage
                        .put_object(Path::new("bench-write.bin"), &payload)
                        .await
                        .unwrap();
                }
            },
            BatchSize::SmallInput,
        )
    });

    group.bench_function("tokio_fs_chunked_read_1_mib", |b| {
        b.to_async(&runtime).iter(|| {
            let storage = fs_storage.clone();
            async move {
                let total = read_all_chunks(
                    &storage,
                    Path::new("bench.bin"),
                    DEFAULT_TRANSFER_CHUNK_SIZE,
                )
                .await
                .unwrap();
                black_box(total);
            }
        })
    });

    group.throughput(Throughput::Bytes(LARGE_STORAGE_OBJECT_SIZE as u64));
    group.bench_function("tokio_fs_put_16_mib", |b| {
        b.to_async(&runtime).iter_batched(
            || {
                let temp_dir = tempfile::tempdir().unwrap();
                let storage = TokioFsStorage::new(temp_dir.path().to_path_buf());
                (temp_dir, storage, large_payload.clone())
            },
            |(temp_dir, storage, payload)| async move {
                let _temp_dir = temp_dir;
                storage
                    .put_object(Path::new("bench-large-write.bin"), &payload)
                    .await
                    .unwrap();
            },
            BatchSize::LargeInput,
        )
    });

    let large_temp_dir = tempfile::tempdir().unwrap();
    let large_fs_storage = TokioFsStorage::new(large_temp_dir.path().to_path_buf());
    runtime
        .block_on(large_fs_storage.put_object(Path::new("bench-large.bin"), &large_payload))
        .unwrap();
    group.bench_function("tokio_fs_chunked_read_16_mib", |b| {
        b.to_async(&runtime).iter(|| {
            let storage = large_fs_storage.clone();
            async move {
                let total = read_all_chunks(
                    &storage,
                    Path::new("bench-large.bin"),
                    DEFAULT_TRANSFER_CHUNK_SIZE,
                )
                .await
                .unwrap();
                black_box(total);
            }
        })
    });

    group.throughput(Throughput::Bytes(
        (MANY_SMALL_OBJECT_COUNT * SMALL_PAYLOAD_SIZE) as u64,
    ));
    group.bench_function("tokio_fs_many_small_put_256x4_kib", |b| {
        b.to_async(&runtime).iter_batched(
            || {
                let temp_dir = tempfile::tempdir().unwrap();
                let storage = TokioFsStorage::new(temp_dir.path().to_path_buf());
                (temp_dir, storage)
            },
            |(temp_dir, storage)| async move {
                let _temp_dir = temp_dir;
                black_box(seed_many_small_objects(&storage).await.unwrap());
            },
            BatchSize::SmallInput,
        )
    });

    let many_small_temp_dir = tempfile::tempdir().unwrap();
    let many_small_fs_storage = TokioFsStorage::new(many_small_temp_dir.path().to_path_buf());
    runtime
        .block_on(seed_many_small_objects(&many_small_fs_storage))
        .unwrap();
    let many_small_paths = many_small_object_paths();
    group.bench_function("tokio_fs_many_small_read_256x4_kib", |b| {
        b.to_async(&runtime).iter(|| {
            let storage = many_small_fs_storage.clone();
            let paths = many_small_paths.clone();
            async move {
                black_box(read_objects(&storage, &paths).await.unwrap());
            }
        })
    });

    group.throughput(Throughput::Bytes(
        (CONCURRENT_OBJECT_COUNT * CONCURRENT_OBJECT_SIZE) as u64,
    ));
    group.bench_function("tokio_fs_concurrent_put_8x1_mib", |b| {
        b.to_async(&runtime).iter_batched(
            || {
                let temp_dir = tempfile::tempdir().unwrap();
                let storage = TokioFsStorage::new(temp_dir.path().to_path_buf());
                (temp_dir, storage)
            },
            |(temp_dir, storage)| async move {
                let _temp_dir = temp_dir;
                black_box(
                    put_concurrent_objects(
                        storage,
                        CONCURRENT_OBJECT_COUNT,
                        CONCURRENT_OBJECT_SIZE,
                    )
                    .await
                    .unwrap(),
                );
            },
            BatchSize::SmallInput,
        )
    });

    let concurrent_temp_dir = tempfile::tempdir().unwrap();
    let concurrent_fs_storage = TokioFsStorage::new(concurrent_temp_dir.path().to_path_buf());
    runtime
        .block_on(put_concurrent_objects(
            concurrent_fs_storage.clone(),
            CONCURRENT_OBJECT_COUNT,
            CONCURRENT_OBJECT_SIZE,
        ))
        .unwrap();
    group.bench_function("tokio_fs_concurrent_read_8x1_mib", |b| {
        b.to_async(&runtime).iter(|| {
            let storage = concurrent_fs_storage.clone();
            async move {
                black_box(
                    read_concurrent_objects(storage, concurrent_object_paths())
                        .await
                        .unwrap(),
                );
            }
        })
    });

    group.finish();
}

fn bench_transfer_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("transfer_pipeline");
    group.throughput(Throughput::Bytes(LARGE_TRANSFER_SIZE as u64));

    let payload = deterministic_payload(LARGE_TRANSFER_SIZE);
    let compression = CompressionManager::new(NullCompressor);
    let crypto = CryptoManager::new(
        AesGcmEncryptor,
        X25519KeyExchanger,
        HkdfSha256Deriver::default(),
    );
    let key = [11; KEY_SIZE];
    let sealed_outputs = sealed_content_chunks(&payload, &compression, &crypto, &key);

    group.bench_function("chunked_upload_encode_seal_1_mib", |b| {
        b.iter(|| {
            let mut offset = 0;

            for chunk in payload.chunks(DEFAULT_TRANSFER_CHUNK_SIZE) {
                let command = FenrisCommand::WriteObjectChunk(TransferChunk {
                    offset,
                    data: chunk.to_vec(),
                    is_last: offset + chunk.len() as u64 == LARGE_TRANSFER_SIZE as u64,
                    total_size: LARGE_TRANSFER_SIZE as u64,
                });
                let encoded =
                    <ProtobufCodec as ProtocolCodec<FenrisCommand>>::encode(&command).unwrap();
                let compressed = compression.compress(&encoded).unwrap();
                let sealed = crypto.seal(&compressed, &key).unwrap();
                black_box(sealed);
                offset += chunk.len() as u64;
            }

            black_box(offset);
        })
    });

    group.bench_function("chunked_download_open_decode_1_mib", |b| {
        b.iter(|| {
            let mut total = 0;

            for sealed in &sealed_outputs {
                let opened = crypto.open(black_box(sealed), &key).unwrap();
                let decompressed = compression.decompress(&opened).unwrap();
                let output: FenrisOutput =
                    <ProtobufCodec as ProtocolCodec<FenrisOutput>>::decode(&decompressed).unwrap();

                if let FenrisOutput::ObjectContentChunk(chunk) = output {
                    total += chunk.data.len();
                }
            }

            black_box(total);
        })
    });

    group.finish();
}

fn sealed_content_chunks(
    payload: &[u8],
    compression: &CompressionManager<NullCompressor>,
    crypto: &CryptoManager<AesGcmEncryptor, X25519KeyExchanger, HkdfSha256Deriver>,
    key: &[u8; KEY_SIZE],
) -> Vec<Vec<u8>> {
    let mut chunks = Vec::new();
    let mut offset = 0;

    for chunk in payload.chunks(DEFAULT_TRANSFER_CHUNK_SIZE) {
        let output = sample_content_output(chunk.len());
        let FenrisOutput::ObjectContentChunk(mut transfer_chunk) = output else {
            unreachable!("sample content output must be chunked")
        };
        transfer_chunk.offset = offset;
        transfer_chunk.total_size = payload.len() as u64;
        transfer_chunk.is_last = offset + chunk.len() as u64 == payload.len() as u64;

        let encoded = <ProtobufCodec as ProtocolCodec<FenrisOutput>>::encode(
            &FenrisOutput::ObjectContentChunk(transfer_chunk),
        )
        .unwrap();
        let compressed = compression.compress(&encoded).unwrap();
        chunks.push(crypto.seal(&compressed, key).unwrap());
        offset += chunk.len() as u64;
    }

    chunks
}

criterion_group!(
    benches,
    bench_protocol_codec,
    bench_frame_codec,
    bench_compression,
    bench_crypto,
    bench_storage,
    bench_transfer_pipeline
);
criterion_main!(benches);
