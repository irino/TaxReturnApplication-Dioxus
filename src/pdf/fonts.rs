//! Resolve a Japanese-capable TrueType font for PDF embedding.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Candidate font paths (first existing wins).
pub fn font_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(p) = std::env::var("TAX_RETURN_FONT") {
        paths.push(PathBuf::from(p));
    }

    // Bundled / project-local override
    paths.push(PathBuf::from("assets/fonts/NotoSansJP-Regular.otf"));
    paths.push(PathBuf::from("assets/fonts/ipag.ttf"));
    paths.push(PathBuf::from("assets/fonts/ipaexg.ttf"));

    // Common Linux packages
    paths.push(PathBuf::from(
        "/usr/share/fonts/opentype/ipafont-gothic/ipag.ttf",
    ));
    paths.push(PathBuf::from(
        "/usr/share/fonts/opentype/ipafont-gothic/ipagp.ttf",
    ));
    paths.push(PathBuf::from(
        "/usr/share/fonts/opentype/ipaexfont-gothic/ipaexg.ttf",
    ));
    paths.push(PathBuf::from(
        "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf",
    ));
    paths.push(PathBuf::from(
        "/usr/share/fonts/truetype/ipafont/ipag.ttf",
    ));

    // macOS
    paths.push(PathBuf::from("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc"));
    paths.push(PathBuf::from("/Library/Fonts/Arial Unicode.ttf"));
    paths.push(PathBuf::from("/System/Library/Fonts/Supplemental/Arial Unicode.ttf"));

    // Windows
    paths.push(PathBuf::from(r"C:\Windows\Fonts\msgothic.ttc"));
    paths.push(PathBuf::from(r"C:\Windows\Fonts\meiryo.ttc"));
    paths.push(PathBuf::from(r"C:\Windows\Fonts\YuGothM.ttc"));

    paths
}

/// First readable font file among search paths.
pub fn find_japanese_font_path() -> Option<PathBuf> {
    font_search_paths().into_iter().find(|p| p.is_file())
}

/// Load font bytes from the resolved path.
pub fn load_japanese_font_bytes() -> io::Result<(PathBuf, Vec<u8>)> {
    let path = find_japanese_font_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "日本語フォントが見つかりません。IPA ゴシック等をインストールするか、\
             環境変数 TAX_RETURN_FONT に TTF/OTF へのパスを設定してください。",
        )
    })?;
    let mut file = File::open(&path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok((path, bytes))
}

/// Whether a path looks like a font collection (printpdf may reject some TTCs).
pub fn is_font_collection(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("ttc"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_paths_non_empty() {
        assert!(!font_search_paths().is_empty());
    }
}
