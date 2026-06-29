use common::{
    DEFAULT_TRANSFER_CHUNK_SIZE, FenrisCommand, FenrisError, FenrisOutput, MemoryStorage,
    ObjectWriteMode, Result, StorageBackend, TransferChunk,
};
use std::path::{Path, PathBuf};
use tokio::task::JoinSet;

pub const SMALL_PAYLOAD_SIZE: usize = 4 * 1024;
pub const CHUNK_PAYLOAD_SIZE: usize = DEFAULT_TRANSFER_CHUNK_SIZE;
pub const LARGE_TRANSFER_SIZE: usize = 1024 * 1024;
pub const LARGE_STORAGE_OBJECT_SIZE: usize = 16 * 1024 * 1024;
pub const MANY_SMALL_OBJECT_COUNT: usize = 256;
pub const CONCURRENT_OBJECT_COUNT: usize = 8;
pub const CONCURRENT_OBJECT_SIZE: usize = 1024 * 1024;

pub fn deterministic_payload(size: usize) -> Vec<u8> {
    (0..size).map(|index| (index % 251) as u8).collect()
}

pub fn compressible_payload(size: usize) -> Vec<u8> {
    b"fenris benchmark payload ".repeat(size.div_ceil(25))[..size].to_vec()
}

pub fn sample_write_command(size: usize) -> FenrisCommand {
    FenrisCommand::WriteObjectChunk(TransferChunk {
        offset: 0,
        data: deterministic_payload(size),
        is_last: true,
        total_size: size as u64,
    })
}

pub fn sample_begin_write(size: usize) -> FenrisCommand {
    FenrisCommand::BeginObjectWrite {
        path: PathBuf::from("bench.bin"),
        mode: ObjectWriteMode::Write,
        total_size: size as u64,
    }
}

pub fn sample_content_output(size: usize) -> FenrisOutput {
    FenrisOutput::ObjectContentChunk(TransferChunk {
        offset: 0,
        data: deterministic_payload(size),
        is_last: true,
        total_size: size as u64,
    })
}

pub async fn seeded_memory_storage(size: usize) -> Result<MemoryStorage> {
    let storage = MemoryStorage::new();
    storage
        .put_object(Path::new("/bench.bin"), &deterministic_payload(size))
        .await?;
    Ok(storage)
}

pub fn storage_object_path(prefix: &str, index: usize) -> PathBuf {
    PathBuf::from(format!("{prefix}-{index:04}.bin"))
}

pub fn many_small_object_paths() -> Vec<PathBuf> {
    storage_object_paths("small-object", MANY_SMALL_OBJECT_COUNT)
}

pub fn concurrent_object_paths() -> Vec<PathBuf> {
    storage_object_paths("concurrent-object", CONCURRENT_OBJECT_COUNT)
}

fn storage_object_paths(prefix: &str, count: usize) -> Vec<PathBuf> {
    (0..count)
        .map(|index| storage_object_path(prefix, index))
        .collect()
}

pub async fn put_objects<S: StorageBackend>(
    storage: &S,
    paths: &[PathBuf],
    payload_size: usize,
) -> Result<usize> {
    let payload = deterministic_payload(payload_size);
    let mut total = 0;

    for path in paths {
        storage.put_object(path, &payload).await?;
        total += payload.len();
    }

    Ok(total)
}

pub async fn read_objects<S: StorageBackend>(storage: &S, paths: &[PathBuf]) -> Result<usize> {
    let mut total = 0;

    for path in paths {
        total += storage.get_object(path).await?.len();
    }

    Ok(total)
}

pub async fn seed_many_small_objects<S: StorageBackend>(storage: &S) -> Result<usize> {
    put_objects(storage, &many_small_object_paths(), SMALL_PAYLOAD_SIZE).await
}

pub async fn put_concurrent_objects<S>(
    storage: S,
    object_count: usize,
    object_size: usize,
) -> Result<usize>
where
    S: StorageBackend + Clone,
{
    let mut tasks = JoinSet::new();

    for index in 0..object_count {
        let storage = storage.clone();
        let path = storage_object_path("concurrent-object", index);
        let payload = deterministic_payload(object_size);
        tasks.spawn(async move {
            storage.put_object(&path, &payload).await?;
            Ok::<usize, FenrisError>(payload.len())
        });
    }

    join_storage_tasks(tasks).await
}

pub async fn read_concurrent_objects<S>(storage: S, paths: Vec<PathBuf>) -> Result<usize>
where
    S: StorageBackend + Clone,
{
    let mut tasks = JoinSet::new();

    for path in paths {
        let storage = storage.clone();
        tasks
            .spawn(async move { Ok::<usize, FenrisError>(storage.get_object(&path).await?.len()) });
    }

    join_storage_tasks(tasks).await
}

async fn join_storage_tasks(mut tasks: JoinSet<Result<usize>>) -> Result<usize> {
    let mut total = 0;

    while let Some(result) = tasks.join_next().await {
        total += result.map_err(|e| {
            FenrisError::FileOperationError(format!("storage benchmark task failed: {e}"))
        })??;
    }

    Ok(total)
}

pub async fn read_all_chunks<S: StorageBackend>(
    storage: &S,
    path: &Path,
    chunk_size: usize,
) -> Result<usize> {
    let mut offset = 0;
    let mut total_read = 0;

    loop {
        let chunk = storage.get_object_chunk(path, offset, chunk_size).await?;
        total_read += chunk.data.len();
        offset += chunk.data.len() as u64;

        if chunk.is_last {
            return Ok(total_read);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_payload_uses_requested_size() {
        let payload = deterministic_payload(300);

        assert_eq!(payload.len(), 300);
        assert_eq!(payload[0], 0);
        assert_eq!(payload[250], 250);
        assert_eq!(payload[251], 0);
    }

    #[test]
    fn compressible_payload_uses_requested_size() {
        let payload = compressible_payload(113);

        assert_eq!(payload.len(), 113);
        assert!(payload.starts_with(b"fenris benchmark"));
    }

    #[test]
    fn sample_write_command_uses_chunk_payload_size() {
        let command = sample_write_command(42);

        assert!(matches!(
            command,
            FenrisCommand::WriteObjectChunk(TransferChunk {
                data,
                total_size: 42,
                is_last: true,
                ..
            }) if data.len() == 42
        ));
    }

    #[test]
    fn sample_content_output_uses_chunk_payload_size() {
        let output = sample_content_output(84);

        assert!(matches!(
            output,
            FenrisOutput::ObjectContentChunk(TransferChunk {
                data,
                total_size: 84,
                is_last: true,
                ..
            }) if data.len() == 84
        ));
    }

    #[tokio::test]
    async fn read_all_chunks_counts_seeded_memory_object() {
        let storage = seeded_memory_storage(CHUNK_PAYLOAD_SIZE + 17)
            .await
            .unwrap();

        let total = read_all_chunks(
            &storage,
            Path::new("/bench.bin"),
            DEFAULT_TRANSFER_CHUNK_SIZE,
        )
        .await
        .unwrap();

        assert_eq!(total, CHUNK_PAYLOAD_SIZE + 17);
    }

    #[test]
    fn storage_object_paths_are_unique_and_stable() {
        let paths = many_small_object_paths();
        let unique = paths.iter().collect::<std::collections::HashSet<_>>();

        assert_eq!(paths.len(), MANY_SMALL_OBJECT_COUNT);
        assert_eq!(unique.len(), MANY_SMALL_OBJECT_COUNT);
        assert_eq!(paths[0], PathBuf::from("small-object-0000.bin"));
        assert_eq!(paths[255], PathBuf::from("small-object-0255.bin"));
    }

    #[tokio::test]
    async fn seed_many_small_objects_reports_expected_total_bytes() {
        let storage = MemoryStorage::new();

        let total = seed_many_small_objects(&storage).await.unwrap();
        let read_total = read_objects(&storage, &many_small_object_paths())
            .await
            .unwrap();

        assert_eq!(total, MANY_SMALL_OBJECT_COUNT * SMALL_PAYLOAD_SIZE);
        assert_eq!(read_total, total);
    }

    #[tokio::test]
    async fn concurrent_fixture_helpers_report_expected_total_bytes() {
        let storage = MemoryStorage::new();

        let written = put_concurrent_objects(
            storage.clone(),
            CONCURRENT_OBJECT_COUNT,
            CONCURRENT_OBJECT_SIZE,
        )
        .await
        .unwrap();
        let read = read_concurrent_objects(storage, concurrent_object_paths())
            .await
            .unwrap();

        assert_eq!(written, CONCURRENT_OBJECT_COUNT * CONCURRENT_OBJECT_SIZE);
        assert_eq!(read, written);
    }
}
