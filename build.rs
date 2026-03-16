use std::io::{BufRead, Read, Write};

fn main() {
    rerun_if_changed_recursive("crowscii-art");

    let mut out_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("compiled_crows.txt")
        .unwrap();

    let dirs = std::fs::read_dir("crowscii-art").unwrap();
    for entry in dirs {
        if entry.is_err() {
            continue;
        }
        let entry = entry.unwrap();
        if !entry.file_type().is_ok_and(|f| f.is_dir()) {
            continue;
        }
        let crow_dir = entry.path();
        let frames = crow_dir.read_dir().unwrap();
        let mut frames = frames.map(|f| f.unwrap()).collect::<Vec<_>>();
        frames.sort_by_key(|f| f.path());
        frames.retain(|f| f.file_name() != "meta");
        let meta_path = crow_dir.join("meta");
        if let Some(mut meta_file) = std::fs::File::open(meta_path).ok() {
            let mut buf = String::new();
            let _ = meta_file.read_to_string(&mut buf);
            out_file.write_all(buf.trim().as_bytes()).unwrap();
        }

        for frame in frames {
            let frame_path = frame.path();
            let frame_file = std::fs::File::open(frame_path).unwrap();
            let reader = std::io::BufReader::new(frame_file);
            let lines = reader.lines();
            for line in lines {
                out_file
                    .write_all(&line.unwrap().chars().map(|c| c as u8).collect::<Vec<_>>())
                    .unwrap();
                out_file.write_all(b"n").unwrap();
            }
            out_file.write_all(b"f").unwrap();
        }
        out_file.write_all(b"c").unwrap();
    }
}

fn rerun_if_changed_recursive(dir: &str) {
    println!("cargo:rerun-if-changed={dir}");
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rerun_if_changed_recursive(&path.to_string_lossy());
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}
