//! `openmouse-gpui --list-supported <file>`: writes the supported devices that
//! are plugged in right now to `<file>` as JSON and exits, without opening a
//! window. The installer runs this to show what it will work with; it is the
//! same detection the Overview page uses, so the two always agree.
//!
//! A file rather than stdout, because the release build has no console.

use serde::Serialize;

pub const ARG: &str = "--list-supported";

#[derive(Serialize)]
struct Entry {
    name: String,
    brand: String,
}

/// The path after [`ARG`], when the app was started that way.
pub fn requested_path() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == ARG {
            return args.next().map(Into::into);
        }
    }
    None
}

/// Lists the devices and writes them to `path`. Writes `[]` if the device
/// runtime cannot be reached, so a caller always finds a readable file.
pub fn write(path: &std::path::Path) {
    let service = crate::HidService::start();
    let entries: Vec<Entry> = match futures::executor::block_on(service.list_candidates()) {
        Ok(Ok(candidates)) => candidates
            .into_iter()
            .map(|candidate| Entry {
                name: candidate.info.product_string,
                brand: candidate.brands.into_iter().next().unwrap_or_default(),
            })
            .collect(),
        _ => Vec::new(),
    };
    let text = serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string());
    let _ = std::fs::write(path, text);
}
