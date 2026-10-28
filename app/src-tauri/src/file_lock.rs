//! An owned flock scope, including duplicated/inherited descriptor lifetimes.
pub(crate) struct ExclusiveFile(std::fs::File);
impl ExclusiveFile {
    pub(crate) fn new(file: std::fs::File) -> std::io::Result<Self> {
        fs2::FileExt::try_lock_exclusive(&file)?;
        Ok(Self(file))
    }
}

impl Drop for ExclusiveFile {
    fn drop(&mut self) {
        // close-on-exec does not close the child's descriptor during the fork→exec window.
        // Explicitly end this scope's flock before closing our descriptor.
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

/// Read-only ownership probe. Multiple probes may coexist, but an actual
/// exclusive management scope blocks every one of them.
pub(crate) struct SharedFile(std::fs::File);
impl SharedFile {
    pub(crate) fn new(file: std::fs::File) -> std::io::Result<Self> {
        fs2::FileExt::try_lock_shared(&file)?;
        Ok(Self(file))
    }
}
impl Drop for SharedFile {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn releasing_the_owner_unlocks_even_while_a_duplicate_descriptor_remains() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("operation.lock");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        // A duplicate models the same open-file description inherited between fork and exec.
        let inherited = file.try_clone().unwrap();
        let owner = ExclusiveFile::new(file).unwrap();
        let competing = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        assert!(ExclusiveFile::new(competing).is_err());
        drop(owner);
        let next = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let _next_owner = ExclusiveFile::new(next)
            .expect("scope ended; an inherited descriptor must not prolong ownership");
        drop(inherited);
    }
}
