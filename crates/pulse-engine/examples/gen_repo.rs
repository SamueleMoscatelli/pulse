//! `gen_repo <DIR> <ASSETS> <FILES>`: writes a synthetic repo for `scripts/bench.sh`.

use std::path::PathBuf;
use std::process::ExitCode;

#[path = "../benches/support/gen_repo.rs"]
mod gen_repo;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let parsed = match args.as_slice() {
        [dir, assets, files] => assets
            .parse()
            .ok()
            .zip(files.parse().ok())
            .map(|(a, f)| (PathBuf::from(dir), a, f)),
        _ => None,
    };
    let Some((dir, assets, files)) = parsed else {
        eprintln!("usage: gen_repo <DIR> <ASSETS> <FILES>");
        return ExitCode::from(2);
    };
    match gen_repo::generate(&dir, assets, files) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
