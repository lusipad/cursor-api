use std::{fs::File, io::Result};

// ── 资源压缩模块 ──────────────────────────────────────────────
#[cfg(not(feature = "use-minified"))]
mod minify {
    use sha2::{Digest, Sha256};
    use std::{collections::HashMap, fs, io::Result, path::Path, process::Command};

    const MARKDOWN_FILES: [&str; 2] = ["README.md", "LICENSE.md"];

    pub fn check_and_install_deps() -> Result<()> {
        let scripts_dir = Path::new("scripts");
        let node_modules = scripts_dir.join("node_modules");

        if !node_modules.exists() {
            println!("cargo:warning=Installing minifier dependencies...");
            let status = Command::new("npm").current_dir(scripts_dir).arg("install").status()?;

            if !status.success() {
                panic!("Failed to install npm dependencies");
            }
            println!("cargo:warning=Dependencies installed successfully");
        }
        Ok(())
    }

    pub fn minify_assets() -> Result<()> {
        let current_hashes = get_files_hash()?;

        if current_hashes.is_empty() {
            println!("cargo:warning=No files to minify");
            return Ok(());
        }

        let saved_hashes = load_saved_hashes()?;

        let files_to_update: Vec<&str> = current_hashes
            .iter()
            .filter(|&(path, hash)| {
                let min_path = get_minified_output_path(path);
                !Path::new(&min_path).exists() || saved_hashes.get(path.as_str()) != Some(hash)
            })
            .map(|(path, _)| path.as_str())
            .collect();

        if files_to_update.is_empty() {
            println!("cargo:warning=No files need to be updated");
            return Ok(());
        }

        println!("cargo:warning=Minifying {} files...", files_to_update.len());
        println!("cargo:warning=Files: {}", files_to_update.join(" "));

        let status =
            Command::new("node").arg("scripts/minify.js").args(&files_to_update).status()?;

        if !status.success() {
            panic!("Asset minification failed");
        }

        save_hashes(&current_hashes)?;
        Ok(())
    }

    // ── 内部辅助 ──────────────────────────────────────────────

    #[inline]
    fn hex_encode<'buf>(bytes: &[u8], buf: &'buf mut [u8]) -> &'buf str {
        use hex_simd::{AsciiCase, Out, encode_as_str};
        encode_as_str(bytes, Out::from_slice(buf), AsciiCase::Lower)
    }

    /// key = 文件路径字符串, value = sha256 hex
    fn get_files_hash() -> Result<HashMap<String, String>> {
        let mut hashes = HashMap::new();

        for &md_file in &MARKDOWN_FILES {
            let path = Path::new(md_file);
            if path.exists() {
                let content = fs::read(path)?;
                let digest = Sha256::new().chain_update(&content).finalize();
                let hash = hex_encode(digest.as_slice(), &mut [0u8; 64]).to_string();
                hashes.insert(md_file.to_owned(), hash);
            }
        }

        Ok(hashes)
    }

    fn load_saved_hashes() -> Result<HashMap<String, String>> {
        let hash_file = Path::new("scripts/.asset-hashes.json");
        if hash_file.exists() {
            let content = fs::read_to_string(hash_file)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(HashMap::new())
        }
    }

    fn save_hashes(hashes: &HashMap<String, String>) -> Result<()> {
        let content = serde_json::to_string_pretty(hashes)?;
        fs::write("scripts/.asset-hashes.json", content)?;
        Ok(())
    }

    /// 返回压缩后的输出路径字符串
    fn get_minified_output_path(file: &str) -> String {
        let path = Path::new(file);

        if MARKDOWN_FILES.contains(&file) {
            // README.md → static/readme.min.html
            let stem = path.file_stem().unwrap().to_string_lossy().to_lowercase();
            format!("static/{stem}.min.html")
        } else {
            let stem = path.file_stem().unwrap().to_string_lossy();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            format!("{stem}.min.{ext}")
        }
    }
}

// ── 版本更新模块 ──────────────────────────────────────────────
#[cfg(all(not(debug_assertions), not(feature = "__preview_locked"), feature = "__preview"))]
mod version {
    use std::{
        fs::{self, File},
        io::{Read, Result, Write},
    };

    /// 读取 VERSION 文件中的版本号。
    /// 文件不存在或解析失败均返回 `1`。
    pub fn read_version_number() -> Result<usize> {
        let mut version = String::with_capacity(4);
        match File::open("VERSION") {
            Ok(mut file) => {
                file.read_to_string(&mut version)?;
                Ok(version.trim().parse().unwrap_or(1))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(1),
            Err(e) => Err(e),
        }
    }

    /// 读取 VERSION 中的数字，加 1 后写回。
    /// 文件不存在或内容无法解析时从 1 开始。
    pub fn update_version_number() -> Result<()> {
        let next = read_version_number()? + 1;
        fs::write("VERSION", next.to_string())?;
        println!("cargo:warning=Version updated to {next}");
        Ok(())
    }
}

// ── 构建信息生成（无条件编译） ────────────────────────────────
fn generate_build_info() -> Result<()> {
    let file = File::create(__build::variables::out_dir().join("build_info.rs"))?;
    __build::BuildInfo.write_to(file)
}

fn generate_platform_info() -> Result<()> {
    let file = File::create(__build::variables::out_dir().join("platform_info.rs"))?;
    __build::PlatformInfo.write_to(file)
}

// ── 入口 ──────────────────────────────────────────────────────
fn main() -> Result<()> {
    // 版本号更新（release + preview 时）
    #[cfg(all(not(debug_assertions), not(feature = "__preview_locked"), feature = "__preview"))]
    version::update_version_number()?;

    // rerun-if-changed 声明
    println!("cargo:rerun-if-changed=scripts/minify.js");
    println!("cargo:rerun-if-changed=README.md");
    println!("cargo:rerun-if-changed=LICENSE.md");

    #[cfg(all(not(debug_assertions), feature = "__preview"))]
    println!("cargo:rerun-if-changed=VERSION");

    // 资源压缩
    #[cfg(not(feature = "use-minified"))]
    {
        minify::check_and_install_deps()?;
        minify::minify_assets()?;
    }

    // 构建信息
    generate_build_info()?;
    generate_platform_info()?;

    Ok(())
}
