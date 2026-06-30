#![no_main]

use libfuzzer_sys::fuzz_target;
use susun_build::Dockerignore;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let dockerignore = Dockerignore::parse(input);
    let _ = dockerignore.is_ignored(std::path::Path::new("context/file.txt"), false);
    let _ = dockerignore.is_ignored(std::path::Path::new("context/dir"), true);
});
