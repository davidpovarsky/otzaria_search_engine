// Used only by explicitly ignored diagnostics, outside the FFI-scanned API module.
// This never creates, repairs, or supplies a default index.
use std::path::{Path, PathBuf};

const FIXTURE_ROOT_ENV: &str = "OTZARIA_DIAGNOSTIC_FIXTURE_ROOT";
const ROOT_PREFIX: &str = "otzaria-diagnostic-fixture-";
const OWNER_MARKER: &str = ".otzaria-disposable-diagnostic-fixture";
const OWNER_DECLARATION: &str = "I own this disposable fixture and authorize diagnostic writes.\n";

pub(crate) fn owned_diagnostic_index_path() -> String {
    let supplied = std::env::var_os(FIXTURE_ROOT_ENV).expect(
        "ignored diagnostic requires OTZARIA_DIAGNOSTIC_FIXTURE_ROOT; no default index is allowed",
    );
    let supplied = PathBuf::from(supplied);
    assert!(
        supplied.is_absolute(),
        "fixture root must be an absolute path"
    );
    let root = supplied
        .canonicalize()
        .expect("explicit disposable fixture root must already exist");
    let temp_root = std::env::temp_dir()
        .canonicalize()
        .expect("temporary directory must exist");
    assert_eq!(
        root.parent(),
        Some(temp_root.as_path()),
        "fixture root must be a direct child of the temporary directory"
    );
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    assert!(
        name.strip_prefix(ROOT_PREFIX).is_some_and(|suffix| {
            suffix.len() >= 12
                && suffix
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        }),
        "fixture root must have a unique otzaria-diagnostic-fixture- name"
    );
    assert_eq!(
        std::fs::read_to_string(root.join(OWNER_MARKER))
            .expect("fixture requires an explicit disposable ownership marker"),
        OWNER_DECLARATION,
        "fixture ownership marker must explicitly authorize diagnostic writes"
    );
    let index = root
        .join("index")
        .canonicalize()
        .expect("owned fixture must already contain an index directory");
    assert_eq!(
        index.parent(),
        Some(root.as_path()),
        "fixture index must not escape its owned root"
    );
    assert_eq!(
        index.file_name(),
        Some(std::ffi::OsStr::new("index")),
        "fixture index must be the original index child"
    );
    assert!(index.is_dir(), "fixture index must be a directory");
    assert!(
        Path::new(&index).join("meta.json").is_file(),
        "fixture must be a prebuilt index; diagnostics must not create one"
    );
    index
        .to_str()
        .expect("fixture index path must be valid UTF-8")
        .to_owned()
}
