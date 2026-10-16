//! Geometry: bounding boxes, margins and `/Rotate` mapping.

/// A PDF rectangle in points: `x0 y0 x1 y1` with `x0 < x1`, `y0 < y1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Box4 {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Box4 {
    pub fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Self { x0, y0, x1, y1 }
    }

    pub fn parse(s: &str, option: &str) -> Result<Self, String> {
        let cleaned: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
        let parts: Vec<&str> = cleaned.split(' ').collect();
        if parts.len() != 4 {
            return Err(format!("Parse error (option {option} \"{s}\")"));
        }
        let mut v = [0f64; 4];
        for (i, p) in parts.iter().enumerate() {
            v[i] = p
                .parse::<f64>()
                .map_err(|_| format!("Parse error (option {option} \"{s}\")"))?;
        }
        Ok(Box4::new(v[0], v[1], v[2], v[3]))
    }

    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }

    /// The smallest box containing both.
    #[allow(dead_code)]
    pub fn union(&self, other: &Box4) -> Box4 {
        Box4::new(
            self.x0.min(other.x0),
            self.y0.min(other.y0),
            self.x1.max(other.x1),
            self.y1.max(other.y1),
        )
    }

    /// The largest box contained in both, or `None` when they do not overlap.
    pub fn intersect(&self, other: &Box4) -> Option<Box4> {
        let b = Box4::new(
            self.x0.max(other.x0),
            self.y0.max(other.y0),
            self.x1.min(other.x1),
            self.y1.min(other.y1),
        );
        if b.width() > 0.0 && b.height() > 0.0 {
            Some(b)
        } else {
            None
        }
    }

    pub fn contains(&self, other: &Box4) -> bool {
        other.x0 >= self.x0 && other.y0 >= self.y0 && other.x1 <= self.x1 && other.y1 <= self.y1
    }

    /// Round outward to whole points — this is what `%%BoundingBox` reports.
    #[allow(dead_code)]
    pub fn rounded_outward(&self) -> Box4 {
        Box4::new(
            self.x0.floor(),
            self.y0.floor(),
            self.x1.ceil(),
            self.y1.ceil(),
        )
    }

    pub fn map(&self, f: impl Fn(f64, f64) -> (f64, f64)) -> Box4 {
        let pts = [
            f(self.x0, self.y0),
            f(self.x1, self.y0),
            f(self.x0, self.y1),
            f(self.x1, self.y1),
        ];
        let mut b = Box4::new(pts[0].0, pts[0].1, pts[0].0, pts[0].1);
        for (x, y) in pts.iter().skip(1) {
            b.x0 = b.x0.min(*x);
            b.y0 = b.y0.min(*y);
            b.x1 = b.x1.max(*x);
            b.y1 = b.y1.max(*y);
        }
        b
    }
}

/// Extra margins, in points, in the order the original tool documents them.
#[derive(Debug, Clone, Copy, Default)]
pub struct Margins {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Margins {
    pub fn parse(s: &str) -> Result<Self, String> {
        let cleaned: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
        if cleaned.is_empty() {
            return Err("Parse error (option --margins)".into());
        }
        let mut nums: Vec<f64> = Vec::new();
        for p in cleaned.split(' ') {
            nums.push(
                p.parse::<f64>()
                    .map_err(|_| "Parse error (option --margins)".to_string())?,
            );
        }
        let err = || "Parse error (option --margins)".to_string();
        Ok(match nums.len() {
            1 => Margins {
                left: nums[0],
                top: nums[0],
                right: nums[0],
                bottom: nums[0],
            },
            2 => Margins {
                left: nums[0],
                top: nums[1],
                right: nums[0],
                bottom: nums[1],
            },
            4 => Margins {
                left: nums[0],
                top: nums[1],
                right: nums[2],
                bottom: nums[3],
            },
            _ => return Err(err()),
        })
    }
}

/// Grow the measured box by the margins.
pub fn apply_margins(base: Box4, m: &Margins) -> Box4 {
    Box4::new(
        base.x0 - m.left,
        base.y0 - m.bottom,
        base.x1 + m.right,
        base.y1 + m.top,
    )
}

pub fn is_empty(b: &Box4) -> bool {
    b.x0 >= b.x1 || b.y0 >= b.y1
}

/// Convert a box measured in *display* space (what Ghostscript's `bbox` device
/// reports: the page as the reader shows it, `/Rotate` already applied) into the
/// page's own, unrotated coordinate system, so it can be stored in `/MediaBox`
/// and `/CropBox`.
///
/// `media` is the effective `/MediaBox`, `rotate` the effective `/Rotate`.
pub fn display_to_page_space(display: Box4, media: &Box4, rotate: i64) -> Box4 {
    let ox = media.x0;
    let oy = media.y0;
    let w = media.width();
    let h = media.height();
    let r = rotate.rem_euclid(360);

    display.map(|dx, dy| {
        // Ghostscript's box is in display space, which is always 0-based and
        // already rotated; rotate it back, then place it on the media origin.
        let px = dx;
        let py = dy;
        let (ux, uy) = match r {
            0 => (px, py),
            90 => (w - py, px),
            180 => (w - px, h - py),
            270 => (py, h - px),
            _ => (px, py),
        };
        (ox + ux, oy + uy)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media() -> Box4 {
        Box4::new(0.0, 0.0, 400.0, 600.0)
    }

    #[test]
    fn margins_follow_left_top_right_bottom() {
        let m = Margins::parse("5 10 15 20").unwrap();
        let b = apply_margins(Box4::new(100.0, 100.0, 200.0, 200.0), &m);
        assert_eq!(b, Box4::new(95.0, 80.0, 215.0, 210.0));
    }

    #[test]
    fn margins_one_and_two_values() {
        assert_eq!(Margins::parse("10").unwrap().bottom, 10.0);
        let m = Margins::parse("5 10").unwrap();
        assert_eq!((m.left, m.top, m.right, m.bottom), (5.0, 10.0, 5.0, 10.0));
    }

    /// Ink drawn at page (49.5, 99.5)..(300.5, 507.4) on a 400x600 page was
    /// reported by Ghostscript as the boxes below (measured 2026-10-16).
    #[test]
    fn rotation_mapping_matches_ghostscript() {
        let cases = [
            (0, (49.499998, 99.499989, 300.500006, 507.401985)),
            (90, (99.499989, 99.499989, 507.401985, 350.499997)),
            (180, (99.499989, 92.609997, 350.499997, 500.500039)),
            (270, (92.537997, 49.499998, 500.500039, 300.500006)),
        ];
        for (rotate, gs) in cases {
            let display = Box4::new(gs.0, gs.1, gs.2, gs.3);
            let page = display_to_page_space(display, &media(), rotate);
            let want = Box4::new(49.5, 99.5, 300.5, 507.4);
            // Ghostscript rounds inside the rotated frame, so a rotated box
            // can come back a few hundredths of a point off the true ink box.
            let tol = 0.1;
            assert!(
                (page.x0 - want.x0).abs() < tol
                    && (page.y0 - want.y0).abs() < tol
                    && (page.x1 - want.x1).abs() < tol
                    && (page.y1 - want.y1).abs() < tol,
                "rotate {rotate}: got {page:?}, want {want:?}"
            );
        }
    }

    #[test]
    fn rotation_map_respects_nonzero_media_origin() {
        let media = Box4::new(10.0, 20.0, 410.0, 620.0);
        let page = display_to_page_space(Box4::new(0.0, 0.0, 400.0, 600.0), &media, 180);
        assert_eq!(page, Box4::new(10.0, 20.0, 410.0, 620.0));
    }

    #[test]
    fn outward_rounding_widens() {
        let b = Box4::new(1.2, 2.9, 3.1, 4.05).rounded_outward();
        assert_eq!(b, Box4::new(1.0, 2.0, 4.0, 5.0));
    }
}
