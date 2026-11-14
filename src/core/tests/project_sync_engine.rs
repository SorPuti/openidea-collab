use openidea_collab_core::project_sync::{
    FileVersion, Manifest, ProjectStore, StagedFile, synchronize,
};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use tempfile::{NamedTempFile, TempDir};

struct RealTestStore {
    root: PathBuf,
}
fn version(path: &Path) -> io::Result<Option<FileVersion>> {
    match fs::read(path) {
        Ok(data) => Ok(Some(FileVersion::from_bytes(&data))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
impl ProjectStore for RealTestStore {
    fn inventory(&mut self) -> io::Result<Manifest> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let path = entry.file_name().to_string_lossy().to_string();
                entries.push((path, version(&entry.path())?.unwrap()));
            }
        }
        Manifest::new(entries).map_err(io::Error::other)
    }
    fn open_version(&mut self, path: &str, expected: FileVersion) -> io::Result<Box<dyn Read>> {
        let path = self.root.join(path);
        if version(&path)? != Some(expected) {
            return Err(io::Error::other("source changed"));
        }
        Ok(Box::new(fs::File::open(path)?))
    }
    fn stage(
        &mut self,
        path: &str,
        expected: Option<FileVersion>,
        incoming: FileVersion,
    ) -> io::Result<Box<dyn StagedFile>> {
        Ok(Box::new(RealTestStage {
            target: self.root.join(path),
            expected,
            incoming,
            temp: NamedTempFile::new_in(&self.root)?,
        }))
    }
    fn remove_if_version(&mut self, path: &str, expected: FileVersion) -> io::Result<()> {
        let path = self.root.join(path);
        if version(&path)? != Some(expected) {
            return Err(io::Error::other("destination changed"));
        }
        fs::remove_file(path)
    }
}
struct RealTestStage {
    target: PathBuf,
    expected: Option<FileVersion>,
    incoming: FileVersion,
    temp: NamedTempFile,
}
impl Write for RealTestStage {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.temp.write(data)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.temp.flush()
    }
}
impl StagedFile for RealTestStage {
    fn commit(mut self: Box<Self>) -> io::Result<()> {
        self.temp.flush()?;
        if version(&self.target)? != self.expected
            || version(self.temp.path())? != Some(self.incoming)
        {
            return Err(io::Error::other("destination changed or corrupt staging"));
        }
        self.temp.persist(&self.target).map_err(|e| e.error)?;
        Ok(())
    }
}
fn stores() -> (TempDir, RealTestStore, RealTestStore) {
    let root = tempfile::tempdir().unwrap();
    let a = root.path().join("a");
    let b = root.path().join("b");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    (root, RealTestStore { root: a }, RealTestStore { root: b })
}
#[test]
fn synchronizes_real_projects_with_binary_updates_and_deletions() {
    let (_temp, mut a, mut b) = stores();
    let binary: Vec<u8> = (0..(3 * 1024 * 1024 + 7))
        .map(|i| (i % 251) as u8)
        .collect();
    fs::write(a.root.join("image.bin"), &binary).unwrap();
    fs::write(a.root.join("old.txt"), b"one").unwrap();
    let initial = synchronize(&mut a, &mut b, None, 4 * 1024 * 1024).unwrap();
    let base = initial.baseline.unwrap();
    assert!(initial.conflicts.is_empty());
    assert_eq!(fs::read(b.root.join("image.bin")).unwrap(), binary);
    fs::remove_file(a.root.join("old.txt")).unwrap();
    fs::write(b.root.join("new.txt"), b"new remotely").unwrap();
    let next = synchronize(&mut a, &mut b, Some(&base), 4 * 1024 * 1024).unwrap();
    assert!(next.conflicts.is_empty());
    assert!(next.baseline.is_some());
    assert!(!b.root.join("old.txt").exists());
    assert_eq!(fs::read(a.root.join("new.txt")).unwrap(), b"new remotely");
}
#[test]
fn conflicting_real_edits_preserve_both_sides_and_do_not_advance_baseline() {
    let (_temp, mut a, mut b) = stores();
    fs::write(a.root.join("x.txt"), b"base").unwrap();
    let base = synchronize(&mut a, &mut b, None, 1000)
        .unwrap()
        .baseline
        .unwrap();
    fs::write(a.root.join("x.txt"), b"left").unwrap();
    fs::write(b.root.join("x.txt"), b"right").unwrap();
    let result = synchronize(&mut a, &mut b, Some(&base), 1000).unwrap();
    assert_eq!(result.conflicts, ["x.txt"]);
    assert!(result.baseline.is_none());
    assert_eq!(fs::read(a.root.join("x.txt")).unwrap(), b"left");
    assert_eq!(fs::read(b.root.join("x.txt")).unwrap(), b"right");
}

struct CorruptSource(RealTestStore);
impl ProjectStore for CorruptSource {
    fn inventory(&mut self) -> io::Result<Manifest> {
        self.0.inventory()
    }
    fn open_version(&mut self, _path: &str, _expected: FileVersion) -> io::Result<Box<dyn Read>> {
        Ok(Box::new(io::Cursor::new(b"malicious".to_vec())))
    }
    fn stage(
        &mut self,
        path: &str,
        expected: Option<FileVersion>,
        incoming: FileVersion,
    ) -> io::Result<Box<dyn StagedFile>> {
        self.0.stage(path, expected, incoming)
    }
    fn remove_if_version(&mut self, path: &str, expected: FileVersion) -> io::Result<()> {
        self.0.remove_if_version(path, expected)
    }
}
#[test]
fn corrupted_source_never_commits_staged_file() {
    let (_temp, a, mut b) = stores();
    fs::write(a.root.join("x.bin"), b"legitimate").unwrap();
    let mut a = CorruptSource(a);
    assert!(synchronize(&mut a, &mut b, None, 1000).is_err());
    assert!(!b.root.join("x.bin").exists());
    assert!(fs::read_dir(&b.root).unwrap().next().is_none());
}

struct RacingStage {
    inner: Box<dyn StagedFile>,
    target: PathBuf,
}
impl Write for RacingStage {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.inner.write(data)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
impl StagedFile for RacingStage {
    fn commit(self: Box<Self>) -> io::Result<()> {
        fs::write(&self.target, b"local edit during transfer")?;
        self.inner.commit()
    }
}
struct RacingDestination(RealTestStore);
impl ProjectStore for RacingDestination {
    fn inventory(&mut self) -> io::Result<Manifest> {
        self.0.inventory()
    }
    fn open_version(&mut self, path: &str, expected: FileVersion) -> io::Result<Box<dyn Read>> {
        self.0.open_version(path, expected)
    }
    fn stage(
        &mut self,
        path: &str,
        expected: Option<FileVersion>,
        incoming: FileVersion,
    ) -> io::Result<Box<dyn StagedFile>> {
        let target = self.0.root.join(path);
        Ok(Box::new(RacingStage {
            inner: self.0.stage(path, expected, incoming)?,
            target,
        }))
    }
    fn remove_if_version(&mut self, path: &str, expected: FileVersion) -> io::Result<()> {
        self.0.remove_if_version(path, expected)
    }
}
#[test]
fn destination_changed_during_transfer_is_not_overwritten() {
    let (_temp, mut a, b) = stores();
    fs::write(a.root.join("x.txt"), b"remote data").unwrap();
    let mut b = RacingDestination(b);
    assert!(synchronize(&mut a, &mut b, None, 1000).is_err());
    assert_eq!(
        fs::read(b.0.root.join("x.txt")).unwrap(),
        b"local edit during transfer"
    );
}

#[test]
fn transfers_more_than_the_old_open_document_limit() {
    let (_temp, mut a, mut b) = stores();
    for i in 0..96 {
        fs::write(a.root.join(format!("file-{i:03}.bin")), [i as u8, 0, 255]).unwrap();
    }
    let result = synchronize(&mut a, &mut b, None, 1000).unwrap();
    assert!(result.conflicts.is_empty());
    assert_eq!(result.baseline.unwrap().files().len(), 96);
    assert_eq!(fs::read_dir(&b.root).unwrap().count(), 96);
}
