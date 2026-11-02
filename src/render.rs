//! Bakes a key's look into a PNG. Icons are `ph:<name>` from the embedded Phosphor set, or `.svg` and
//! `.png` files; relative paths resolve against the config.
use crate::config::{Crop, Visual, HOLD_STEPS};
use anyhow::{anyhow, Context, Result};
use resvg::tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};
use resvg::usvg;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

const SIZE: u32 = 196;
/// The firmware gives the wide cell this width when it shows an image.
const WIDE_WIDTH: u32 = 458;
const ICONS: &str = include_str!("../assets/phosphor.tsv");
pub const LIBRARY_PREFIX: &str = "ph:";

/// Every embedded icon as (name, inner SVG). Names end in `-bold` or `-fill`.
pub fn library() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    MAP.get_or_init(|| ICONS.lines().filter_map(|line| line.split_once('\t')).collect())
}
const ICON_BOX: f32 = 100.0;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub struct Renderer {
    options: usvg::Options<'static>,
    dir: PathBuf,
    cache: HashMap<(Visual, bool), Vec<u8>>,
}

impl Renderer {
    pub fn new(dir: PathBuf) -> Self {
        let mut options = usvg::Options::default();
        options.fontdb_mut().load_system_fonts();
        Self { options, dir, cache: HashMap::new() }
    }

    pub fn forget_images(&mut self) {
        self.cache.clear();
    }

    pub fn png(&mut self, visual: &Visual, wide: bool) -> Result<&[u8]> {
        let key = (visual.clone(), wide);
        if !self.cache.contains_key(&key) {
            let png = self.draw(visual, wide)?;
            self.cache.insert(key.clone(), png);
        }
        Ok(&self.cache[&key])
    }

    fn svg(&self, source: &str, canvas: &mut Pixmap, transform: Transform) -> Result<usvg::Size> {
        let tree = usvg::Tree::from_str(source, &self.options)?;
        resvg::render(&tree, transform, &mut canvas.as_mut());
        Ok(tree.size())
    }

    fn href(&self, file: &str) -> String {
        escape(&self.dir.join(file).to_string_lossy()).replace('"', "&quot;")
    }

    /// The picture stretched over a unit square, of which the crop is shown in the given box. The
    /// editor keeps the crop at the box's shape, so nothing ends up squashed.
    fn cropped(&self, file: &str, Crop([x, y, w, h]): Crop, place: &str) -> String {
        format!(
            r#"<svg {place} viewBox="{x} {y} {w} {h}" preserveAspectRatio="none"><image href="{}" width="1" height="1" preserveAspectRatio="none"/></svg>"#,
            self.href(file)
        )
    }

    fn icon(&self, visual: &Visual, canvas: &mut Pixmap, y: f32) -> Result<()> {
        let path = self.dir.join(&visual.icon);
        let x = (canvas.width() as f32 - ICON_BOX) / 2.0;
        let named = visual.icon.strip_prefix(LIBRARY_PREFIX);
        if named.is_some() || path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg")) {
            let source = match named {
                Some(name) => {
                    let inner = library().get(name).ok_or_else(|| anyhow!("no icon named {name}"))?;
                    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256" fill="currentColor">{inner}</svg>"#)
                }
                None => std::fs::read_to_string(&path).with_context(|| path.display().to_string())?,
            };
            let source = source.replace("currentColor", &visual.color);
            let tree = usvg::Tree::from_str(&source, &self.options)?;
            let scale = ICON_BOX / tree.size().width().max(tree.size().height());
            let at = Transform::from_translate(x, y).pre_scale(scale, scale);
            resvg::render(&tree, at, &mut canvas.as_mut());
        } else if let Some(crop) = visual.icon_crop {
            let place = format!(r#"x="{x}" y="{y}" width="{ICON_BOX}" height="{ICON_BOX}""#);
            let source = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{SIZE}">{}</svg>"#,
                canvas.width(),
                self.cropped(&visual.icon, crop, &place)
            );
            self.svg(&source, canvas, Transform::identity())?;
        } else {
            let image = Pixmap::load_png(&path).with_context(|| path.display().to_string())?;
            let scale = ICON_BOX / image.width().max(image.height()) as f32;
            let paint = PixmapPaint { quality: FilterQuality::Bicubic, ..Default::default() };
            let at = Transform::from_translate(x, y).pre_scale(scale, scale);
            canvas.draw_pixmap(0, 0, image.as_ref(), &paint, at, None);
        }
        Ok(())
    }

    fn draw(&self, v: &Visual, wide: bool) -> Result<Vec<u8>> {
        let width = if wide { WIDE_WIDTH } else { SIZE };

        let mut canvas = Pixmap::new(width, SIZE).ok_or_else(|| anyhow!("canvas"))?;
        let text = (!v.label.is_empty()).then(|| {
            format!(
                r##"<text x="{}" y="178" text-anchor="middle" font-family="Segoe UI, sans-serif" font-size="27" font-weight="600" fill="#ffffff">{}</text>"##,
                width / 2,
                escape(&v.label)
            )
        });
        let big = (!v.big.is_empty()).then(|| {
            format!(
                r##"<text x="{}" y="118" text-anchor="middle" font-family="Segoe UI, sans-serif" font-size="64" font-weight="700" fill="{}">{}</text>"##,
                width / 2,
                v.color,
                escape(&v.big)
            )
        });
        // usvg reads PNG, JPEG, GIF and WebP, and skips a file it cannot load.
        let image = (!v.image.is_empty()).then(|| match v.image_crop {
            Some(crop) => self.cropped(&v.image, crop, &format!(r#"width="{width}" height="{SIZE}""#)),
            None => format!(
                r#"<image href="{}" width="100%" height="100%" preserveAspectRatio="xMidYMid slice"/>"#,
                self.href(&v.image)
            ),
        });
        let background = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{SIZE}"><rect width="100%" height="100%" fill="{}"/>{}{}{}</svg>"#,
            v.background,
            image.as_deref().unwrap_or(""),
            big.as_deref().unwrap_or(""),
            text.as_deref().unwrap_or("")
        );
        self.svg(&background, &mut canvas, Transform::identity())?;

        if !v.icon.is_empty() {
            let y = if v.label.is_empty() { 48.0 } else { 30.0 };
            // A missing icon should not take the whole page down with it.
            if let Err(e) = self.icon(v, &mut canvas, y) {
                eprintln!("icon {}: {e:#}", v.icon);
            }
        }
        if v.progress > 0 {
            // Stroke a dash along the rectangle, which starts at the top left going clockwise.
            const INSET: f32 = 7.0;
            const RADIUS: f32 = 24.0;
            let (w, h) = (width as f32 - INSET * 2.0, SIZE as f32 - INSET * 2.0);
            let perimeter = 2.0 * (w + h) - 8.0 * RADIUS + std::f32::consts::TAU * RADIUS;
            // The last frame is a plain outline, so no gap can remain where the dash starts.
            let dash = match v.progress {
                HOLD_STEPS.. => String::new(),
                step => format!(r#"stroke-dasharray="{} {perimeter}""#, perimeter * step as f32 / HOLD_STEPS as f32),
            };
            let border = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{SIZE}"><rect x="{INSET}" y="{INSET}" width="{w}" height="{h}" rx="{RADIUS}" fill="none" stroke="{}" stroke-width="12" {dash}/></svg>"#,
                v.color
            );
            self.svg(&border, &mut canvas, Transform::identity())?;
        }
        Ok(canvas.encode_png()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixmap(png: &[u8]) -> Pixmap {
        Pixmap::decode_png(png).unwrap()
    }

    #[test]
    fn draws_icon_and_escaped_label() {
        let dir = std::env::temp_dir().join("openulanzi-render-test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("i.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><rect width="24" height="24"/></svg>"#,
        )
        .unwrap();
        let mut r = Renderer::new(dir);
        let v = Visual {
            label: "A&B <1>".into(),
            icon: "i.svg".into(),
            color: "#ff0000".into(),
            background: "#000000".into(),
            ..Default::default()
        };
        let image = pixmap(r.png(&v, false).unwrap());
        assert_eq!((image.width(), image.height()), (196, 196));
        let center = image.pixel(98, 80).unwrap();
        assert_eq!((center.red(), center.green(), center.blue()), (255, 0, 0), "currentColor uses the key colour");
        let text_row = (0..196).any(|x| image.pixel(x, 170).unwrap().green() > 128);
        assert!(text_row, "label must render, so a system font was found");
    }

    #[test]
    fn library_icons_render_in_the_key_colour() {
        assert!(library().len() > 3000);
        let mut r = Renderer::new(std::env::temp_dir());
        let v = Visual { icon: "ph:square-fill".into(), color: "#00ff00".into(), background: "#000000".into(), ..Default::default() };
        let image = pixmap(r.png(&v, false).unwrap());
        assert_eq!(image.pixel(98, 98).unwrap().green(), 255);
    }

    #[test]
    fn hold_border_grows_clockwise_from_the_top_left() {
        let mut r = Renderer::new(std::env::temp_dir());
        let visual = |progress| Visual { color: "#ff0000".into(), background: "#000000".into(), progress, ..Default::default() };
        let half = pixmap(r.png(&visual(HOLD_STEPS / 2), false).unwrap());
        assert_eq!(half.pixel(98, 7).unwrap().red(), 255, "top edge is drawn");
        assert_eq!(half.pixel(188, 98).unwrap().red(), 255, "right edge is drawn");
        assert_eq!(half.pixel(7, 98).unwrap().red(), 0, "left edge is not yet");
        let full = pixmap(r.png(&visual(HOLD_STEPS), false).unwrap());
        assert_eq!(full.pixel(7, 98).unwrap().red(), 255, "all the way round");
        assert_eq!(full.pixel(32, 7).unwrap().red(), 255, "closing the loop where it began");
    }

    #[test]
    fn big_text_draws_in_the_middle() {
        let mut r = Renderer::new(std::env::temp_dir());
        let v = Visual { big: "1/2".into(), color: "#ffffff".into(), background: "#000000".into(), ..Default::default() };
        let image = pixmap(r.png(&v, false).unwrap());
        assert!((40..156).any(|x| image.pixel(x, 96).unwrap().red() > 128));
    }

    #[test]
    fn background_image_fills_the_key_under_the_label() {
        let dir = std::env::temp_dir().join("openulanzi-bg-test");
        std::fs::create_dir_all(&dir).unwrap();
        let mut red = Pixmap::new(40, 20).unwrap();
        red.fill(resvg::tiny_skia::Color::from_rgba8(255, 0, 0, 255));
        red.save_png(dir.join("a & b.png")).unwrap();
        let mut r = Renderer::new(dir);
        let v = Visual { image: "a & b.png".into(), label: "x".into(), background: "#000000".into(), ..Default::default() };
        let image = pixmap(r.png(&v, false).unwrap());
        let corner = image.pixel(2, 2).unwrap();
        assert_eq!((corner.red(), corner.green()), (255, 0), "covers the key, cropping the sides");
        let missing = Visual { image: "missing.jpg".into(), background: "#000000".into(), ..Default::default() };
        assert!(r.png(&missing, false).is_ok());
    }

    #[test]
    fn crops_pick_the_part_of_a_picture_shown() {
        let dir = std::env::temp_dir().join("openulanzi-crop-test");
        std::fs::create_dir_all(&dir).unwrap();
        // Left half red, right half blue.
        let mut halves = Pixmap::new(40, 20).unwrap();
        halves.fill(resvg::tiny_skia::Color::from_rgba8(0, 0, 255, 255));
        let red = resvg::tiny_skia::Rect::from_xywh(0.0, 0.0, 20.0, 20.0).unwrap();
        let paint = resvg::tiny_skia::Paint { shader: resvg::tiny_skia::Shader::SolidColor(resvg::tiny_skia::Color::from_rgba8(255, 0, 0, 255)), ..Default::default() };
        halves.fill_rect(red, &paint, Transform::identity(), None);
        halves.save_png(dir.join("halves.png")).unwrap();
        let mut r = Renderer::new(dir);
        let right = Some(Crop([0.5, 0.0, 0.5, 1.0]));
        let v = Visual { image: "halves.png".into(), image_crop: right, background: "#000000".into(), ..Default::default() };
        let image = pixmap(r.png(&v, false).unwrap());
        assert_eq!(image.pixel(20, 98).unwrap().blue(), 255, "the left edge shows the right half");
        let v = Visual { icon: "halves.png".into(), icon_crop: right, background: "#000000".into(), ..Default::default() };
        let image = pixmap(r.png(&v, false).unwrap());
        let center = image.pixel(98, 98).unwrap();
        assert_eq!((center.red(), center.blue()), (0, 255), "the icon shows only the right half");
    }

    #[test]
    fn wide_uses_the_full_cell_and_bad_icon_is_survivable() {
        let mut r = Renderer::new(std::env::temp_dir());
        let v = Visual {
            icon: "does-not-exist.png".into(),
            color: "#ffffff".into(),
            background: "#102030".into(),
            ..Default::default()
        };
        let image = pixmap(r.png(&v, true).unwrap());
        assert_eq!((image.width(), image.height()), (458, 196));
        assert_eq!(image.pixel(190, 10).unwrap().blue(), 0x30);
    }
}
