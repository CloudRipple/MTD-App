use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

fn main() {
    println!("cargo:rerun-if-env-changed=MTD_EMBED_FFMPEG_DIR");
    println!("cargo:rerun-if-env-changed=MTD_EMBED_UI_FONT");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let output_path = out_dir.join("embedded_assets.rs");
    let ffmpeg_dir = env::var_os("MTD_EMBED_FFMPEG_DIR").map(PathBuf::from);
    let ui_font = env::var_os("MTD_EMBED_UI_FONT")
        .map(PathBuf::from)
        .or_else(|| {
            let default = PathBuf::from("assets")
                .join("fonts")
                .join("HarmonyOS_Sans_SC_Regular.ttf");
            default.exists().then_some(default)
        })
        .map(|path| path.canonicalize().unwrap_or(path));

    let mut code = String::new();
    let ffmpeg_files = ffmpeg_dir
        .as_deref()
        .filter(|path| path.exists())
        .map(collect_files)
        .unwrap_or_default();
    let ffmpeg_fingerprint = fingerprint_files(&ffmpeg_files);
    code.push_str(&format!(
        "pub(crate) const FFMPEG_FINGERPRINT: &str = \"{ffmpeg_fingerprint:016x}\";\n"
    ));
    code.push_str("pub(crate) const FFMPEG_FILES: &[EmbeddedFile] = &[\n");
    for file in &ffmpeg_files {
        println!("cargo:rerun-if-changed={}", file.display());
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .expect("embedded file name");
        code.push_str(&format!(
            "    EmbeddedFile {{ name: {:?}, bytes: include_bytes!({:?}) }},\n",
            name,
            file.display().to_string()
        ));
    }
    code.push_str("];\n");

    match ui_font.as_deref().filter(|path| path.exists()) {
        Some(path) => {
            println!("cargo:rerun-if-changed={}", path.display());
            let fingerprint = fingerprint_files(&[path.to_path_buf()]);
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("HarmonyOS_Sans_SC_Regular.ttf");
            code.push_str(&format!(
                "pub(crate) const UI_FONT_FINGERPRINT: &str = \"{fingerprint:016x}\";\n"
            ));
            code.push_str(&format!(
                "pub(crate) const UI_FONT_FILE_NAME: &str = {file_name:?};\n"
            ));
            code.push_str(&format!(
                "pub(crate) const UI_FONT_BYTES: Option<&'static [u8]> = Some(include_bytes!({:?}));\n",
                path.display().to_string()
            ));
        }
        None => {
            code.push_str("pub(crate) const UI_FONT_FINGERPRINT: &str = \"none\";\n");
            code.push_str(
                "pub(crate) const UI_FONT_FILE_NAME: &str = \"HarmonyOS_Sans_SC_Regular.ttf\";\n",
            );
            code.push_str("pub(crate) const UI_FONT_BYTES: Option<&'static [u8]> = None;\n");
        }
    }

    fs::write(output_path, code).expect("write embedded assets");
}

fn collect_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(dir)
        .expect("read embed dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| {
                    extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("dll")
                })
                .unwrap_or(false)
        })
        .map(|path| path.canonicalize().unwrap_or(path))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn fingerprint_files(files: &[PathBuf]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for path in files {
        path.file_name().hash(&mut hasher);
        match fs::read(path) {
            Ok(bytes) => bytes.hash(&mut hasher),
            Err(_) => path.display().to_string().hash(&mut hasher),
        }
    }
    hasher.finish()
}
