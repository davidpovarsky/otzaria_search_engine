# Exact pre-writer schema-failure source trace

This trace reads source files and retained logs only. It does not reopen the user index.

## Original engine at 265a14ea54f959a3e266d6e8acfa3a3b98101d68

```text
1723:     pub fn new(path: &str) -> Self {
1724:         init_engine_logger();
1725:         debug!("new path={}", path);
1726:         let schema = current_schema();
1727:         let mmap_directory = MmapDirectory::open(path).expect("unable to open mmap directory");
1728:         let index = match Index::open_or_create(mmap_directory, schema.clone()) {
1729:             Ok(index) => index,
1730:             Err(tantivy::TantivyError::SchemaError(err)) => panic!(
1731:                 "index at {path} was built with an incompatible schema ({err}); \
1732:                  call check_index_compatibility before opening and rebuild the index"
1733:             ),
1734:             Err(err) => panic!("Failed to open index at {path}: {err}"),
1735:         };
1736:         // אנליזטורי השדות (מצב אינדוקס): מילה עם גרש/גרשיים מוטמעת גם
1737:         // בצורתה הנקייה באותה עמדה — חיפוש `רמבם` מוצא `רמב"ם`.
1738:         // RemoveLongFilter על *כל* האנליזטורים — כולל צד השאילתה, אחרת
```

```text
1785:             .reader_builder()
1786:             .reload_policy(ReloadPolicy::OnCommitWithDelay)
1787:             .try_into()
1788:             .expect("Failed to create index reader");
1789:         // Best-effort: if another instance/process holds the writer lock right
1790:         // now, start without a writer; ensure_writer() retries on first write.
1791:         let index_writer = match index.writer(DEFAULT_WRITER_HEAP_SIZE) {
1792:             Ok(writer) => Some(writer),
1793:             Err(err) => {
1794:                 warn!("writer unavailable at startup ({err}); will retry lazily");
1795:                 None
1796:             }
1797:         };
1798:
1799:         if let Err(err) = ensure_current_index_metadata(Path::new(path)) {
1800:             debug!("failed to ensure index metadata: {err:#}");
1801:         }
1802:
1803:         SearchEngine {
```

## Tantivy 0.26.1: src/index/index.rs

```text
218:     pub fn open_or_create<T: Into<Box<dyn Directory>>>(self, dir: T) -> crate::Result<Index> {
219:         let dir: Box<dyn Directory> = dir.into();
220:         if !Index::exists(&*dir)? {
221:             return self.create(dir);
222:         }
223:         let mut index = Index::open(dir)?;
224:         index.set_tokenizers(self.tokenizer_manager.clone());
225:         if index.schema() == self.get_expect_schema()? {
226:             Ok(index)
227:         } else {
228:             Err(TantivyError::SchemaError(
229:                 "An index exists but the schema does not match.".to_string(),
230:             ))
231:         }
```

```text
285:     pub fn exists(dir: &dyn Directory) -> Result<bool, OpenReadError> {
286:         dir.exists(&META_FILEPATH)
287:     }
288:
289:     /// Accessor to the search executor.
```

```text
510:     pub fn open<T: Into<Box<dyn Directory>>>(directory: T) -> crate::Result<Index> {
511:         let directory = directory.into();
512:         let directory = ManagedDirectory::wrap(directory)?;
513:         let inventory = SegmentMetaInventory::default();
514:         let metas = load_metas(&directory, &inventory)?;
515:         let index = Index::open_from_metas(directory, &metas, inventory);
516:         Ok(index)
```

## Tantivy 0.26.1: src/directory/managed_directory.rs

```text
64:     pub fn wrap(directory: Box<dyn Directory>) -> crate::Result<ManagedDirectory> {
65:         match directory.atomic_read(&MANAGED_FILEPATH) {
66:             Ok(data) => {
67:                 let managed_files_json = String::from_utf8_lossy(&data);
68:                 let managed_files: HashSet<PathBuf> = serde_json::from_str(&managed_files_json)
69:                     .map_err(|e| {
70:                         DataCorruption::new(
71:                             MANAGED_FILEPATH.to_path_buf(),
72:                             format!("Managed file cannot be deserialized: {e:?}. "),
73:                         )
74:                     })?;
75:                 Ok(ManagedDirectory {
76:                     directory,
77:                     meta_informations: Arc::new(RwLock::new(MetaInformation {
78:                         managed_paths: managed_files,
79:                     })),
80:                 })
81:             }
82:             Err(OpenReadError::FileDoesNotExist(_)) => Ok(ManagedDirectory {
83:                 directory,
84:                 meta_informations: Arc::default(),
85:             }),
86:             io_err @ Err(OpenReadError::IoError { .. }) => Err(io_err.err().unwrap().into()),
87:             Err(OpenReadError::IncompatibleIndex(incompatibility)) => {
88:                 // For the moment, this should never happen  `meta.json`
89:                 // do not have any footer and cannot detect incompatibility.
90:                 Err(crate::TantivyError::IncompatibleIndex(incompatibility))
```

## Tantivy 0.26.1: src/directory/mmap_directory/mod.rs

```text
236:     pub fn open(directory_path: impl AsRef<Path>) -> Result<MmapDirectory, OpenDirectoryError> {
237:         Self::open_impl_to_avoid_monomorphization(directory_path.as_ref())
238:     }
239:
240:     #[inline(never)]
241:     fn open_impl_to_avoid_monomorphization(
242:         directory_path: &Path,
243:     ) -> Result<MmapDirectory, OpenDirectoryError> {
244:         if !directory_path.exists() {
245:             return Err(OpenDirectoryError::DoesNotExist(PathBuf::from(
246:                 directory_path,
247:             )));
248:         }
249:         #[expect(clippy::bind_instead_of_map)]
250:         let canonical_path: PathBuf = directory_path.canonicalize().or_else(|io_err| {
251:             let directory_path = directory_path.to_owned();
252:
253:             #[cfg(windows)]
254:             {
255:                 // `canonicalize` returns "Incorrect function" (error code 1)
256:                 // for virtual drives (network drives, ramdisk, etc.).
257:                 if io_err.raw_os_error() == Some(1) && directory_path.exists() {
258:                     // Should call `std::path::absolute` when it is stabilised.
259:                     return Ok(directory_path);
260:                 }
261:             }
262:
263:             Err(OpenDirectoryError::wrap_io_error(io_err, directory_path))
264:         })?;
265:         if !canonical_path.is_dir() {
266:             return Err(OpenDirectoryError::NotADirectory(PathBuf::from(
267:                 directory_path,
268:             )));
269:         }
270:         Ok(MmapDirectory::new(canonical_path, None))
```

```text
451:     fn atomic_read(&self, path: &Path) -> Result<Vec<u8>, OpenReadError> {
452:         let full_path = self.resolve_path(path);
453:         let mut buffer = Vec::new();
454:         match File::open(full_path) {
455:             Ok(mut file) => {
456:                 file.read_to_end(&mut buffer).map_err(|io_error| {
457:                     OpenReadError::wrap_io_error(io_error, path.to_path_buf())
458:                 })?;
459:                 Ok(buffer)
460:             }
461:             Err(io_error) => {
462:                 if io_error.kind() == io::ErrorKind::NotFound {
463:                     Err(OpenReadError::FileDoesNotExist(path.to_owned()))
464:                 } else {
465:                     Err(OpenReadError::wrap_io_error(io_error, path.to_path_buf()))
466:                 }
467:             }
468:         }
```

## Interpretation and observation limits

The recorded incompatible-schema errors occur at Index::open_or_create, before the original writer constructor and index-metadata upgrade call. The existing-meta branch checks the schema before returning the opened index. The shown directory/metadata operations read existing files. This source ordering does not establish byte-for-byte before/after identity or absence of access-time changes. No before-state was captured. The after-only observation is retained separately; no user-index repair or subsequent opening was used to make tests pass.
