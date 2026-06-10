//! Renders the tray usage-meter icon with cairo, mirroring the macOS
//! IconRenderer: a small meter whose fill represents percent remaining (or
//! percent used when flipped), dimmed when the last refresh failed, with an
//! incident dot overlaid when a provider status indicates an outage.

use crate::model::RateWindow;
use crate::providers::Rgb;

/// ARGB32 pixel buffer for an icon, ready to hand to ksni.
pub struct IconPixmap {
    pub width: i32,
    pub height: i32,
    /// Row-major ARGB32 (premultiplied), as ksni expects.
    pub data: Vec<u8>,
}

pub struct IconOptions {
    /// When true, the bar fills with *used* percent; default fills *remaining*.
    pub show_as_used: bool,
    /// Dim the icon when the most recent refresh failed / data is stale.
    pub dimmed: bool,
    /// Overlay an incident dot (status minor/major/critical).
    pub incident: bool,
    pub accent: Rgb,
}

impl Default for IconOptions {
    fn default() -> Self {
        IconOptions {
            show_as_used: false,
            dimmed: false,
            incident: false,
            accent: Rgb { r: 0.2, g: 0.7, b: 0.5 },
        }
    }
}

const SIZE: i32 = 22; // panel icon; HiDPI handled by GNOME scaling

/// Render the meter for a single window. `None` windows render an empty/unknown
/// meter (outline only).
pub fn render(window: Option<&RateWindow>, opts: &IconOptions) -> IconPixmap {
    let surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, SIZE, SIZE).unwrap();
    {
        let cr = cairo::Context::new(&surface).unwrap();
        let alpha = if opts.dimmed { 0.45 } else { 1.0 };

        // Meter geometry: a rounded vertical-ish bar inset from the edges.
        let inset = 3.0;
        let w = SIZE as f64 - inset * 2.0;
        let h = SIZE as f64 - inset * 2.0;
        let x = inset;
        let y = inset;
        let radius = 4.0;

        // Track (background outline).
        rounded_rect(&cr, x, y, w, h, radius);
        cr.set_source_rgba(0.5, 0.5, 0.5, 0.35 * alpha);
        cr.set_line_width(1.5);
        let _ = cr.stroke_preserve();
        cr.set_source_rgba(0.5, 0.5, 0.5, 0.12 * alpha);
        let _ = cr.fill();

        // Fill fraction.
        if let Some(win) = window {
            let frac = if opts.show_as_used {
                win.used_percent / 100.0
            } else {
                win.remaining_percent() / 100.0
            }
            .clamp(0.0, 1.0);

            // Fill from the bottom up to `frac` of the height.
            let fill_h = h * frac;
            let fy = y + (h - fill_h);
            rounded_rect_clip(&cr, x, y, w, h, radius);
            let a = &opts.accent;
            // Color shifts toward red as remaining gets low (visual urgency),
            // independent of accent for at-a-glance reading.
            let low = win.remaining_percent() <= 10.0 && !opts.show_as_used;
            if low {
                cr.set_source_rgba(0.85, 0.20, 0.20, alpha);
            } else {
                cr.set_source_rgba(a.r, a.g, a.b, alpha);
            }
            cr.rectangle(x, fy, w, fill_h);
            let _ = cr.fill();
            cr.reset_clip();
        }

        // Incident dot (top-right).
        if opts.incident {
            cr.set_source_rgba(0.90, 0.30, 0.10, alpha);
            cr.arc(SIZE as f64 - 4.5, 4.5, 3.0, 0.0, std::f64::consts::TAU);
            let _ = cr.fill();
        }
    }

    let width = surface.width();
    let height = surface.height();
    let stride = surface.stride();
    let mut data = surface.take_data().unwrap().to_vec();

    // cairo ARgb32 is native-endian BGRA premultiplied; ksni wants ARGB bytes.
    // On little-endian, cairo bytes are [B,G,R,A]; convert to [A,R,G,B].
    let mut argb = Vec::with_capacity((width * height * 4) as usize);
    for row in 0..height {
        let base = (row * stride) as usize;
        for col in 0..width {
            let px = base + (col * 4) as usize;
            let b = data[px];
            let g = data[px + 1];
            let r = data[px + 2];
            let a = data[px + 3];
            argb.push(a);
            argb.push(r);
            argb.push(g);
            argb.push(b);
        }
    }
    data.clear();

    IconPixmap { width, height, data: argb }
}

fn rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let deg = std::f64::consts::PI / 180.0;
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -90.0 * deg, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, 90.0 * deg);
    cr.arc(x + r, y + h - r, r, 90.0 * deg, 180.0 * deg);
    cr.arc(x + r, y + r, r, 180.0 * deg, 270.0 * deg);
    cr.close_path();
}

fn rounded_rect_clip(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    rounded_rect(cr, x, y, w, h, r);
    cr.clip();
}
