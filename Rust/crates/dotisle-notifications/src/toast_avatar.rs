use std::{
    fs::File,
    io::{Cursor, Read},
    os::windows::fs::MetadataExt,
    path::{Component, Path, PathBuf, Prefix},
};
use windows::{core::{PCWSTR, PWSTR}, Win32::UI::Shell::PathCreateFromUrlW};

const MAX_BYTES: u64 = 512 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum App { WeChat, Qq }

fn app_for_id(id: &str) -> Option<App> {
    let id = id.to_ascii_lowercase();
    let file = id.rsplit(['\\', '/']).next()?;
    match (id.as_str(), file) {
        ("qq" | "qqnt" | "tencent.qq" | "tencent.qq.nt", _) | (_, "qq.exe") => Some(App::Qq),
        ("wechat" | "weixin" | "tencent.wechat" | "tencent.weixin", _) |
        (_, "wechat.exe" | "weixin.exe") => Some(App::WeChat),
        _ => None,
    }
}

fn avatar_src<'a, 'input>(binding: roxmltree::Node<'a, 'input>) -> Option<&'a str> {
    binding.children().find(|image| {
        image.has_tag_name("image") && image.attribute("placement") == Some("appLogoOverride")
    })?.attribute("src")
}

pub fn from_binding(id: &str, binding: roxmltree::Node<'_, '_>) -> Option<Vec<u8>> {
    let app = app_for_id(id)?;
    let path = local_image_path(avatar_src(binding)?)?;
    let roots = cache_roots(app);
    load_avatar(&path, &roots)
}

fn local_image_path(src: &str) -> Option<PathBuf> {
    if src.len() > 4096 || src.contains(['\0', '\r', '\n']) { return None; }
    let path = if src.get(..8).is_some_and(|s| s.eq_ignore_ascii_case("file:///")) {
        // Do not pass remote authorities or embedded credentials to the shell parser.
        let uri = src.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let mut out = [0u16; 4096];
        let mut size = out.len() as u32;
        unsafe { PathCreateFromUrlW(PCWSTR(uri.as_ptr()), PWSTR(out.as_mut_ptr()), &mut size, 0).ok()?; }
        PathBuf::from(String::from_utf16(&out[..size as usize]).ok()?)
    } else {
        PathBuf::from(src)
    };
    let mut parts = path.components();
    if !matches!(parts.next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_))) ||
        parts.next() != Some(Component::RootDir) ||
        parts.any(|p| !matches!(p, Component::Normal(s) if !s.to_string_lossy().contains(':'))) {
        return None;
    }
    Some(path)
}

fn cache_roots(app: App) -> Vec<PathBuf> {
    let names = match app {
        App::WeChat => &["WeChat", "Weixin"][..],
        App::Qq => &["QQ", "QQNT"][..],
    };
    let mut roots = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        for name in names {
            roots.push(local.join("Tencent").join(name));
            roots.push(local.join("Temp").join("Tencent").join(name));
            roots.push(local.join("Temp").join(name));
        }
    }
    if let Some(roaming) = std::env::var_os("APPDATA").map(PathBuf::from) {
        roots.extend(names.iter().map(|name| roaming.join("Tencent").join(name)));
    }
    roots
}

fn load_avatar(path: &Path, roots: &[PathBuf]) -> Option<Vec<u8>> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(extension.as_str(), "png" | "jpg" | "jpeg") { return None; }
    let lower = |p: &Path| p.to_string_lossy().replace('/', "\\").to_lowercase();
    let candidate = PathBuf::from(lower(path));
    if !roots.iter().any(|root| candidate.starts_with(PathBuf::from(lower(root)))) { return None; }
    // Avoid following junctions, symlinks and cloud placeholders in attachment paths.
    let mut ancestor = PathBuf::new();
    for part in path.components() {
        ancestor.push(part.as_os_str());
        if matches!(part, Component::Normal(_)) && ancestor.symlink_metadata().ok()?.file_attributes() & 0x400 != 0 {
            return None;
        }
    }
    let mut bytes = Vec::new();
    let file = File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() { return None; }
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_BYTES { return None; }
    decode_avatar(&bytes)
}

fn decode_avatar(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().ok()?;
    if !matches!(reader.format(), Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)) { return None; }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(1024);
    limits.max_image_height = Some(1024);
    limits.max_alloc = Some(8 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().ok()?.thumbnail(64, 64);
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_qq_and_wechat_replacement_logos_are_candidates() {
        assert!(app_for_id("Tencent.QQ.NT") == Some(App::Qq));
        assert!(app_for_id(r"C:\QQNT\QQ.exe") == Some(App::Qq));
        assert!(app_for_id("Tencent.Weixin") == Some(App::WeChat));
        assert!(app_for_id("Tencent.QQMusic").is_none());
        assert!(app_for_id("FakeWeChat").is_none());
        let doc = roxmltree::Document::parse(r#"<binding><image placement="hero" src="other.jpg"/><image placement="appLogoOverride" src="sender.jpg"/></binding>"#).unwrap();
        assert_eq!(avatar_src(doc.root_element()), Some("sender.jpg"));
        let inline = roxmltree::Document::parse(r#"<binding><image src="inline.png"/></binding>"#).unwrap();
        assert_eq!(avatar_src(inline.root_element()), None);
    }

    #[test]
    fn refuses_remote_relative_and_unsafe_paths() {
        for src in ["https://example.com/avatar.jpg", "file://host/share/avatar.png", r"\\host\share\avatar.png", r"C:\cache\..\outside.png", r"C:\cache\avatar.png:stream", "avatar.png"] {
            assert!(local_image_path(src).is_none(), "{src}");
        }
        assert_eq!(local_image_path("file:///C:/cache/my%20avatar.png"), Some(PathBuf::from(r"C:\cache\my avatar.png")));
        assert!(load_avatar(Path::new(r"C:\elsewhere\avatar.png"), &[PathBuf::from(r"C:\cache")]).is_none());
    }

    #[test]
    fn normalizes_real_png_and_jpeg_to_small_icons() {
        let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(256, 256, image::Rgb([220, 30, 10])));
        for format in [image::ImageFormat::Jpeg, image::ImageFormat::Png] {
            let mut input = Cursor::new(Vec::new());
            image.write_to(&mut input, format).unwrap();
            let out = image::load_from_memory(&decode_avatar(input.get_ref()).unwrap()).unwrap();
            assert_eq!((out.width(), out.height()), (64, 64));
        }
        assert!(decode_avatar(b"not an image").is_none());
    }
}
