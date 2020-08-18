use std::fs::File;
use std::io::{BufReader, prelude::*};
use std::path::Path;

pub struct Playlist {
    cwd: String,
    files: Vec<String>,
    cursor: usize,
}

impl Playlist {
    pub fn new_with_single_item(file: String) -> Self {
        let path = Path::new(&file);

        Playlist {
            cwd: if path.is_absolute() { String::from("") } else { String::from("./") },
            files: vec![file],
            cursor: 0
        }
    }

    pub fn new_from_m3u(m3u_file: String) -> Self {
        let path = Path::new(&m3u_file);
        let cwd = String::from(path.parent().unwrap_or(Path::new(".")).to_str().unwrap()) + "/";

        let file = File::open(m3u_file).expect("Cannot open m3u file");
        let reader = BufReader::new(file);

        let mut files: Vec<String> = vec![];
        for (_, line) in reader.lines().enumerate() {
            let line = line.unwrap();
            if !line.starts_with("#") {
                files.push(line);
            }
        }

        Playlist {
            cwd: cwd,
            files: files,
            cursor: 0
        }
    }

    pub fn next_file(&mut self) -> String {
        let result = self.cwd.clone() + &self.files[self.cursor].clone();
        self.cursor = if self.cursor == self.files.len() - 1 { 0 } else { self.cursor + 1 };
        result
    }
}