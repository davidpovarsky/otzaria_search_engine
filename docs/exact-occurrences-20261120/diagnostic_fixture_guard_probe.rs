// Filesystem-only test of the actual opt-in guard. No SearchEngine/Tantivy is linked.
// The placeholder meta.json is never opened as an index.
#[path = "../../rust/src/diagnostic_test_fixture.rs"]
mod diagnostic_test_fixture;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct OwnedProbeRoot(PathBuf);
impl Drop for OwnedProbeRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn rejects(label: &str) {
    assert!(
        std::panic::catch_unwind(diagnostic_test_fixture::owned_diagnostic_index_path).is_err(),
        "guard unexpectedly accepted: {label}"
    );
    println!("PASS rejection: {label}");
}

fn main() {
    // This is an isolated single-threaded process. Restore inherited selection on exit.
    let previous = std::env::var_os("OTZARIA_DIAGNOSTIC_FIXTURE_ROOT");
    std::panic::set_hook(Box::new(|_| {}));
    std::env::remove_var("OTZARIA_DIAGNOSTIC_FIXTURE_ROOT");
    rejects("missing explicit environment path");
    std::env::set_var("OTZARIA_DIAGNOSTIC_FIXTURE_ROOT", "relative-fixture");
    rejects("relative path");
    std::env::set_var("OTZARIA_DIAGNOSTIC_FIXTURE_ROOT", std::env::temp_dir());
    rejects("temporary directory itself is not an owned fixture");
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let root =
        OwnedProbeRoot(std::env::temp_dir().join(format!("otzaria-diagnostic-fixture-{suffix}")));
    std::fs::create_dir(&root.0).unwrap();
    std::env::set_var("OTZARIA_DIAGNOSTIC_FIXTURE_ROOT", &root.0);
    rejects("missing ownership marker");
    let marker = root.0.join(".otzaria-disposable-diagnostic-fixture");
    std::fs::write(&marker, "not an authorization\n").unwrap();
    rejects("incorrect ownership declaration");
    std::fs::write(
        &marker,
        "I own this disposable fixture and authorize diagnostic writes.\n",
    )
    .unwrap();
    rejects("index must already exist");
    std::fs::create_dir(root.0.join("index")).unwrap();
    rejects("index metadata must already exist");
    std::fs::write(
        root.0.join("index/meta.json"),
        b"{\"guardProbeOnly\":true}\n",
    )
    .unwrap();
    let accepted = diagnostic_test_fixture::owned_diagnostic_index_path();
    assert_eq!(
        PathBuf::from(accepted),
        root.0.join("index").canonicalize().unwrap()
    );
    println!("PASS acceptance: explicitly owned disposable fixture path");
    if let Some(value) = previous {
        std::env::set_var("OTZARIA_DIAGNOSTIC_FIXTURE_ROOT", value);
    } else {
        std::env::remove_var("OTZARIA_DIAGNOSTIC_FIXTURE_ROOT");
    }
    println!("guard checks: 8 passed; 0 failed; no engine/index open");
}
