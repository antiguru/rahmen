use image::DynamicImage;

struct DirectoryProvider {
    paths: Vec<std::path::PathBuf>,
}

impl DirectoryProvider {
    pub fn new(paths: &[&str]) -> Self {
        // let paths = paths
        //     .into_iter()
        //     .map(Path::try_from)
        //     .filter(Result::is_ok)
        //     .map(Result::unwrap)
        //     .collect();
        Self { paths: vec![] }
    }

    pub fn next_entry(&mut self) -> DynamicImage {
        panic!();
    }
}
