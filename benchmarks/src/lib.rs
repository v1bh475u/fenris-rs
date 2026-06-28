use common::{
    DEFAULT_TRANSFER_CHUNK_SIZE, FenrisCommand, FenrisOutput, MemoryStorage, ObjectWriteMode,
    Result, StorageBackend, TransferChunk,
};
use std::path::{Path, PathBuf};

pub const SMALL_PAYLOAD_SIZE: usize = 4 * 1024;
pub const CHUNK_PAYLOAD_SIZE: usize = DEFAULT_TRANSFER_CHUNK_SIZE;
pub const LARGE_TRANSFER_SIZE: usize = 1024 * 1024;

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
}
