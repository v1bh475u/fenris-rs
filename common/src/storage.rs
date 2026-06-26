use crate::{DefaultFileOperations, FenrisError, FenrisMetadata, FileOperations, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

#[async_trait::async_trait]
pub trait StorageBackend: Send + Sync + 'static {
    async fn put_object(&self, path: &Path, data: &[u8]) -> Result<()>;

    async fn get_object(&self, path: &Path) -> Result<Vec<u8>>;

    async fn append_object(&self, path: &Path, data: &[u8]) -> Result<()>;

    async fn delete_object(&self, path: &Path) -> Result<()>;

    async fn metadata(&self, path: &Path) -> Result<FenrisMetadata>;

    async fn create_namespace(&self, path: &Path) -> Result<()>;

    async fn list_namespace(&self, path: &Path) -> Result<Vec<FenrisMetadata>>;

    async fn delete_namespace(&self, path: &Path) -> Result<()>;

    async fn exists(&self, path: &Path) -> bool;

    async fn is_namespace(&self, path: &Path) -> bool;

    async fn is_object(&self, path: &Path) -> bool;
}

#[derive(Debug, Clone)]
pub struct TokioFsStorage {
    file_ops: DefaultFileOperations,
}

impl TokioFsStorage {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            file_ops: DefaultFileOperations::new(base_dir),
        }
    }

    pub fn with_file_ops(file_ops: DefaultFileOperations) -> Self {
        Self { file_ops }
    }

    pub fn base_dir(&self) -> &Path {
        self.file_ops.base_dir()
    }
}

#[async_trait::async_trait]
impl StorageBackend for TokioFsStorage {
    async fn put_object(&self, path: &Path, data: &[u8]) -> Result<()> {
        self.file_ops.write_file(path, data).await
    }

    async fn get_object(&self, path: &Path) -> Result<Vec<u8>> {
        self.file_ops.read_file(path).await
    }

    async fn append_object(&self, path: &Path, data: &[u8]) -> Result<()> {
        self.file_ops.append_file(path, data).await
    }

    async fn delete_object(&self, path: &Path) -> Result<()> {
        self.file_ops.delete_file(path).await
    }

    async fn metadata(&self, path: &Path) -> Result<FenrisMetadata> {
        self.file_ops
            .file_info(path)
            .await
            .map(FenrisMetadata::from)
    }

    async fn create_namespace(&self, path: &Path) -> Result<()> {
        self.file_ops.create_dir(path).await
    }

    async fn list_namespace(&self, path: &Path) -> Result<Vec<FenrisMetadata>> {
        Ok(self
            .file_ops
            .list_dir(path)
            .await?
            .into_iter()
            .map(FenrisMetadata::from)
            .collect())
    }

    async fn delete_namespace(&self, path: &Path) -> Result<()> {
        self.file_ops.delete_dir(path).await
    }

    async fn exists(&self, path: &Path) -> bool {
        self.file_ops.exists(path).await
    }

    async fn is_namespace(&self, path: &Path) -> bool {
        self.file_ops.is_dir(path).await
    }

    async fn is_object(&self, path: &Path) -> bool {
        self.file_ops.is_file(path).await
    }
}

#[derive(Debug, Clone)]
pub struct MemoryStorage {
    state: Arc<Mutex<MemoryStorageState>>,
}

#[derive(Debug)]
struct MemoryStorageState {
    objects: HashMap<PathBuf, Vec<u8>>,
    namespaces: HashSet<PathBuf>,
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for MemoryStorageState {
    fn default() -> Self {
        Self {
            objects: HashMap::new(),
            namespaces: HashSet::from([PathBuf::from("/")]),
        }
    }
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(MemoryStorageState::default())),
        }
    }

    fn normalize_path(path: &Path) -> Result<PathBuf> {
        let mut normalized = PathBuf::from("/");

        for component in path.components() {
            match component {
                Component::RootDir | Component::CurDir => {}
                Component::Normal(name) => normalized.push(name),
                Component::ParentDir | Component::Prefix(_) => {
                    return Err(FenrisError::FileOperationError(
                        "Path outside storage root".to_string(),
                    ));
                }
            }
        }

        Ok(normalized)
    }

    fn parent_namespace(path: &Path) -> Option<PathBuf> {
        path.parent().map(|parent| {
            if parent.as_os_str().is_empty() {
                PathBuf::from("/")
            } else {
                parent.to_path_buf()
            }
        })
    }

    fn metadata_for(path: &Path, size: u64, is_namespace: bool) -> FenrisMetadata {
        FenrisMetadata {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            size,
            is_namespace,
            modified_time: 0,
            permissions: if is_namespace { 0o755 } else { 0o644 },
        }
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, MemoryStorageState>> {
        self.state.lock().map_err(|_| {
            FenrisError::FileOperationError("Memory storage lock poisoned".to_string())
        })
    }

    fn ensure_parent_namespace(state: &MemoryStorageState, path: &Path) -> Result<()> {
        if Self::parent_namespace(path)
            .as_ref()
            .is_some_and(|parent| state.namespaces.contains(parent))
        {
            return Ok(());
        }

        Err(FenrisError::FileOperationError(
            "Parent namespace not found".to_string(),
        ))
    }
}

#[async_trait::async_trait]
impl StorageBackend for MemoryStorage {
    async fn put_object(&self, path: &Path, data: &[u8]) -> Result<()> {
        let path = Self::normalize_path(path)?;
        let mut state = self.lock_state()?;

        if path == Path::new("/") || state.namespaces.contains(&path) {
            return Err(FenrisError::FileOperationError(
                "Path is a namespace".to_string(),
            ));
        }

        Self::ensure_parent_namespace(&state, &path)?;
        state.objects.insert(path, data.to_vec());
        Ok(())
    }

    async fn get_object(&self, path: &Path) -> Result<Vec<u8>> {
        let path = Self::normalize_path(path)?;
        self.lock_state()?
            .objects
            .get(&path)
            .cloned()
            .ok_or_else(|| FenrisError::FileOperationError("Object not found".to_string()))
    }

    async fn append_object(&self, path: &Path, data: &[u8]) -> Result<()> {
        let path = Self::normalize_path(path)?;
        let mut state = self.lock_state()?;

        if path == Path::new("/") || state.namespaces.contains(&path) {
            return Err(FenrisError::FileOperationError(
                "Path is a namespace".to_string(),
            ));
        }

        Self::ensure_parent_namespace(&state, &path)?;
        state
            .objects
            .entry(path)
            .or_default()
            .extend_from_slice(data);
        Ok(())
    }

    async fn delete_object(&self, path: &Path) -> Result<()> {
        let path = Self::normalize_path(path)?;
        if self.lock_state()?.objects.remove(&path).is_some() {
            return Ok(());
        }

        Err(FenrisError::FileOperationError(
            "Object not found".to_string(),
        ))
    }

    async fn metadata(&self, path: &Path) -> Result<FenrisMetadata> {
        let path = Self::normalize_path(path)?;
        let state = self.lock_state()?;

        if let Some(data) = state.objects.get(&path) {
            return Ok(Self::metadata_for(&path, data.len() as u64, false));
        }

        if state.namespaces.contains(&path) {
            return Ok(Self::metadata_for(&path, 0, true));
        }

        Err(FenrisError::FileOperationError(
            "Path not found".to_string(),
        ))
    }

    async fn create_namespace(&self, path: &Path) -> Result<()> {
        let path = Self::normalize_path(path)?;
        let mut state = self.lock_state()?;

        if state.objects.contains_key(&path) {
            return Err(FenrisError::FileOperationError(
                "Path is an object".to_string(),
            ));
        }

        if path != Path::new("/") {
            Self::ensure_parent_namespace(&state, &path)?;
        }

        state.namespaces.insert(path);
        Ok(())
    }

    async fn list_namespace(&self, path: &Path) -> Result<Vec<FenrisMetadata>> {
        let path = Self::normalize_path(path)?;
        let state = self.lock_state()?;

        if !state.namespaces.contains(&path) {
            return Err(FenrisError::FileOperationError(
                "Namespace not found".to_string(),
            ));
        }

        let mut entries = Vec::new();

        for namespace in &state.namespaces {
            if namespace != &path && namespace.parent() == Some(path.as_path()) {
                entries.push(Self::metadata_for(namespace, 0, true));
            }
        }

        for (object, data) in &state.objects {
            if object.parent() == Some(path.as_path()) {
                entries.push(Self::metadata_for(object, data.len() as u64, false));
            }
        }

        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }

    async fn delete_namespace(&self, path: &Path) -> Result<()> {
        let path = Self::normalize_path(path)?;
        let mut state = self.lock_state()?;

        if path == Path::new("/") {
            return Err(FenrisError::FileOperationError(
                "Cannot delete root namespace".to_string(),
            ));
        }

        if !state.namespaces.contains(&path) {
            return Err(FenrisError::FileOperationError(
                "Namespace not found".to_string(),
            ));
        }

        let has_children = state
            .namespaces
            .iter()
            .any(|namespace| namespace != &path && namespace.starts_with(&path))
            || state.objects.keys().any(|object| object.starts_with(&path));

        if has_children {
            return Err(FenrisError::FileOperationError(
                "Namespace is not empty".to_string(),
            ));
        }

        state.namespaces.remove(&path);
        Ok(())
    }

    async fn exists(&self, path: &Path) -> bool {
        let Ok(path) = Self::normalize_path(path) else {
            return false;
        };

        self.state.lock().is_ok_and(|state| {
            state.objects.contains_key(&path) || state.namespaces.contains(&path)
        })
    }

    async fn is_namespace(&self, path: &Path) -> bool {
        let Ok(path) = Self::normalize_path(path) else {
            return false;
        };

        self.state
            .lock()
            .is_ok_and(|state| state.namespaces.contains(&path))
    }

    async fn is_object(&self, path: &Path) -> bool {
        let Ok(path) = Self::normalize_path(path) else {
            return false;
        };

        self.state
            .lock()
            .is_ok_and(|state| state.objects.contains_key(&path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct TestBackend<S> {
        storage: S,
        _temp_dir: Option<TempDir>,
    }

    fn tokio_fs_storage() -> TestBackend<TokioFsStorage> {
        let temp_dir = TempDir::new().unwrap();
        let storage = TokioFsStorage::new(temp_dir.path().to_path_buf());
        TestBackend {
            storage,
            _temp_dir: Some(temp_dir),
        }
    }

    fn memory_storage() -> TestBackend<MemoryStorage> {
        TestBackend {
            storage: MemoryStorage::new(),
            _temp_dir: None,
        }
    }

    async fn assert_put_and_get_object_round_trip<S: StorageBackend>(storage: &S) {
        storage
            .put_object(Path::new("data.txt"), b"hello")
            .await
            .unwrap();
        let data = storage.get_object(Path::new("data.txt")).await.unwrap();

        assert_eq!(data, b"hello");
    }

    async fn assert_put_object_overwrites_existing_object<S: StorageBackend>(storage: &S) {
        storage
            .put_object(Path::new("data.txt"), b"first")
            .await
            .unwrap();
        storage
            .put_object(Path::new("data.txt"), b"second")
            .await
            .unwrap();

        let data = storage.get_object(Path::new("data.txt")).await.unwrap();
        assert_eq!(data, b"second");
    }

    async fn assert_append_object_extends_existing_object<S: StorageBackend>(storage: &S) {
        storage
            .put_object(Path::new("log.txt"), b"first")
            .await
            .unwrap();
        storage
            .append_object(Path::new("log.txt"), b" second")
            .await
            .unwrap();

        let data = storage.get_object(Path::new("log.txt")).await.unwrap();
        assert_eq!(data, b"first second");
    }

    async fn assert_append_object_creates_missing_object_when_parent_exists<S: StorageBackend>(
        storage: &S,
    ) {
        storage.create_namespace(Path::new("logs")).await.unwrap();
        storage
            .append_object(Path::new("logs/today.txt"), b"entry")
            .await
            .unwrap();

        let data = storage
            .get_object(Path::new("logs/today.txt"))
            .await
            .unwrap();
        assert_eq!(data, b"entry");
    }

    async fn assert_delete_object_removes_object<S: StorageBackend>(storage: &S) {
        storage
            .put_object(Path::new("data.txt"), b"hello")
            .await
            .unwrap();
        assert!(storage.exists(Path::new("data.txt")).await);

        storage.delete_object(Path::new("data.txt")).await.unwrap();

        assert!(!storage.exists(Path::new("data.txt")).await);
    }

    async fn assert_metadata_reports_object_and_namespace_shape<S: StorageBackend>(storage: &S) {
        storage
            .put_object(Path::new("data.txt"), b"hello")
            .await
            .unwrap();
        storage.create_namespace(Path::new("docs")).await.unwrap();

        let object = storage.metadata(Path::new("data.txt")).await.unwrap();
        assert_eq!(object.name, "data.txt");
        assert_eq!(object.size, 5);
        assert!(!object.is_namespace);

        let namespace = storage.metadata(Path::new("docs")).await.unwrap();
        assert_eq!(namespace.name, "docs");
        assert!(namespace.is_namespace);
    }

    async fn assert_namespace_create_list_and_delete<S: StorageBackend>(storage: &S) {
        storage.create_namespace(Path::new("docs")).await.unwrap();
        storage
            .put_object(Path::new("docs/a.txt"), b"a")
            .await
            .unwrap();
        storage
            .create_namespace(Path::new("docs/nested"))
            .await
            .unwrap();

        let entries = storage.list_namespace(Path::new("docs")).await.unwrap();
        let names: Vec<String> = entries.into_iter().map(|entry| entry.name).collect();
        assert!(names.contains(&"a.txt".to_string()));
        assert!(names.contains(&"nested".to_string()));

        storage
            .delete_namespace(Path::new("docs/nested"))
            .await
            .unwrap();
        assert!(!storage.exists(Path::new("docs/nested")).await);
    }

    async fn assert_existence_and_kind_checks_reflect_storage_state<S: StorageBackend>(
        storage: &S,
    ) {
        storage
            .put_object(Path::new("data.txt"), b"hello")
            .await
            .unwrap();
        storage.create_namespace(Path::new("docs")).await.unwrap();

        assert!(storage.exists(Path::new("data.txt")).await);
        assert!(storage.is_object(Path::new("data.txt")).await);
        assert!(!storage.is_namespace(Path::new("data.txt")).await);

        assert!(storage.exists(Path::new("docs")).await);
        assert!(storage.is_namespace(Path::new("docs")).await);
        assert!(!storage.is_object(Path::new("docs")).await);
    }

    async fn assert_path_traversal_is_rejected<S: StorageBackend>(storage: &S) {
        let result = storage.get_object(Path::new("../../../etc/passwd")).await;

        assert!(matches!(result, Err(FenrisError::FileOperationError(_))));
    }

    macro_rules! storage_contract_tests {
        ($module:ident, $storage:ident) => {
            mod $module {
                use super::*;

                #[tokio::test]
                async fn put_and_get_object_round_trip() {
                    let backend = $storage();
                    assert_put_and_get_object_round_trip(&backend.storage).await;
                }

                #[tokio::test]
                async fn put_object_overwrites_existing_object() {
                    let backend = $storage();
                    assert_put_object_overwrites_existing_object(&backend.storage).await;
                }

                #[tokio::test]
                async fn append_object_extends_existing_object() {
                    let backend = $storage();
                    assert_append_object_extends_existing_object(&backend.storage).await;
                }

                #[tokio::test]
                async fn append_object_creates_missing_object_when_parent_exists() {
                    let backend = $storage();
                    assert_append_object_creates_missing_object_when_parent_exists(
                        &backend.storage,
                    )
                    .await;
                }

                #[tokio::test]
                async fn delete_object_removes_object() {
                    let backend = $storage();
                    assert_delete_object_removes_object(&backend.storage).await;
                }

                #[tokio::test]
                async fn metadata_reports_object_and_namespace_shape() {
                    let backend = $storage();
                    assert_metadata_reports_object_and_namespace_shape(&backend.storage).await;
                }

                #[tokio::test]
                async fn namespace_create_list_and_delete() {
                    let backend = $storage();
                    assert_namespace_create_list_and_delete(&backend.storage).await;
                }

                #[tokio::test]
                async fn existence_and_kind_checks_reflect_storage_state() {
                    let backend = $storage();
                    assert_existence_and_kind_checks_reflect_storage_state(&backend.storage).await;
                }

                #[tokio::test]
                async fn path_traversal_is_rejected() {
                    let backend = $storage();
                    assert_path_traversal_is_rejected(&backend.storage).await;
                }
            }
        };
    }

    storage_contract_tests!(tokio_fs_storage_contract, tokio_fs_storage);
    storage_contract_tests!(memory_storage_contract, memory_storage);

    #[tokio::test]
    async fn memory_storage_uses_logical_absolute_paths() {
        let storage = MemoryStorage::new();

        storage
            .put_object(Path::new("data.txt"), b"hello")
            .await
            .unwrap();

        let data = storage.get_object(Path::new("/data.txt")).await.unwrap();
        assert_eq!(data, b"hello");
    }

    #[tokio::test]
    async fn memory_storage_rejects_traversal_on_writes() {
        let storage = MemoryStorage::new();

        let result = storage
            .put_object(Path::new("../../../outside.txt"), b"nope")
            .await;

        assert!(matches!(result, Err(FenrisError::FileOperationError(_))));
        assert!(!storage.exists(Path::new("outside.txt")).await);
    }
}
