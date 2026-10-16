//! Lossless page-box rewriting.
//!
//! The cropped file differs from the input only in the page dictionaries: the
//! content streams, resources, annotations, outlines and metadata are left
//! exactly as they were. That is what makes this writer cheaper (and lossless)
//! compared to re-embedding every page through pdfTeX.

use crate::pagebox::Box4;
use lopdf::{Dictionary, Document as Pdf, Object, ObjectId};
use std::path::Path;

pub struct Document {
    pdf: Pdf,
    /// Page object ids, in page order.
    pages: Vec<ObjectId>,
}

impl Document {
    pub fn open(path: &Path) -> Result<Self, String> {
        let pdf = Pdf::load(path).map_err(|e| format!("cannot open `{}': {e}", path.display()))?;
        // get_pages() is keyed by *page number*, so iterating it yields the pages
        // in reading order. Never sort the ObjectIds: a file whose page objects
        // are not in ascending object order (common in assembled documents) would
        // then get each page's bounding box applied to a different page.
        let pages: Vec<ObjectId> = pdf.get_pages().into_values().collect();
        if pages.is_empty() {
            return Err(format!("`{}' has no pages", path.display()));
        }
        Ok(Self { pdf, pages })
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Effective `/MediaBox` of page `index` (1-based), following `/Parent`.
    pub fn media_box(&self, index: usize) -> Result<Box4, String> {
        let id = self.page_id(index)?;
        let obj = self
            .inherited(id, b"MediaBox")
            .ok_or_else(|| format!("page {index} has no MediaBox"))?;
        array_to_box(&obj).ok_or_else(|| format!("page {index} has an invalid MediaBox"))
    }

    /// Effective `/Rotate` of page `index` (1-based), following `/Parent`.
    pub fn rotate(&self, index: usize) -> Result<i64, String> {
        let id = self.page_id(index)?;
        Ok(self
            .inherited(id, b"Rotate")
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0)
            .rem_euclid(360))
    }

    /// Set `/MediaBox` and `/CropBox` of page `index` (1-based) to `b`.
    ///
    /// `/TrimBox`, `/BleedBox` and `/ArtBox` are clamped to the new page so the
    /// file cannot keep a print box that falls outside its own page.
    pub fn set_page_box(&mut self, index: usize, b: Box4, hires: bool) -> Result<(), String> {
        let id = self.page_id(index)?;
        let array = box_to_array(b, hires);
        let dict = self
            .pdf
            .get_dictionary_mut(id)
            .map_err(|e| format!("page {index}: {e}"))?;
        dict.set("MediaBox", array.clone());
        dict.set("CropBox", array.clone());
        for key in ["TrimBox", "BleedBox", "ArtBox"] {
            let existing = match dict.get(key.as_bytes()) {
                Ok(o) => array_to_box(o),
                Err(_) => None,
            };
            if let Some(existing) = existing {
                if !b.contains(&existing) {
                    let clamped = existing.intersect(&b).unwrap_or(b);
                    dict.set(key.as_bytes(), box_to_array(clamped, hires));
                }
            }
        }
        Ok(())
    }

    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        self.pdf
            .save(path)
            .map_err(|e| format!("cannot write `{}': {e}", path.display()))?;
        Ok(())
    }

    fn page_id(&self, index: usize) -> Result<ObjectId, String> {
        self.pages
            .get(index.saturating_sub(1))
            .copied()
            .ok_or_else(|| format!("page {index} does not exist"))
    }

    /// Look a key up on the page, then on each `/Parent` in turn.
    fn inherited(&self, id: ObjectId, key: &[u8]) -> Option<Object> {
        let mut current = Some(id);
        let mut guard = 0;
        while let Some(id) = current {
            guard += 1;
            if guard > 64 {
                return None;
            }
            let dict = self.pdf.get_dictionary(id).ok()?;
            if let Ok(obj) = dict.get(key) {
                return Some(obj.clone());
            }
            current = match dict.get(b"Parent") {
                Ok(Object::Reference(r)) => Some(*r),
                _ => None,
            };
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Document as Pdf};

    /// Real files can list page objects whose object numbers do not follow the
    /// page order (found in a D:\ARCHITECTURE sheet on 2026-10-16: sorting the
    /// page ids silently applied every page's bounding box to the wrong page).
    /// Page order must always come from the page tree.
    #[test]
    fn page_order_follows_the_page_tree_not_object_numbers() {
        let mut pdf = Pdf::with_version("1.4");
        // Deliberately create the page objects in reverse: the last page gets the
        // lowest object number.
        let mut ids = Vec::new();
        for i in 0..3 {
            let id = pdf.new_object_id();
            pdf.objects.insert(
                id,
                Object::Dictionary(dictionary! {
                    "Type" => "Page",
                    "MediaBox" => vec![0.into(), 0.into(), (100 + 10 * i).into(), 200.into()],
                }),
            );
            ids.push(id);
        }
        ids.reverse(); // object numbers now run opposite to page order
        let pages_id = pdf.new_object_id();
        let kids: Vec<Object> = ids.iter().map(|id| Object::Reference(*id)).collect();
        pdf.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => kids,
                "Count" => 3,
            }),
        );
        for id in &ids {
            if let Ok(Object::Dictionary(d)) = pdf.get_object_mut(*id) {
                d.set("Parent", Object::Reference(pages_id));
            }
        }
        let catalog_id = pdf.new_object_id();
        pdf.objects.insert(
            catalog_id,
            Object::Dictionary(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages_id) }),
        );
        pdf.trailer.set("Root", Object::Reference(catalog_id));

        let dir = std::env::temp_dir();
        let path = dir.join("pdfcrop_page_order_test.pdf");
        pdf.save(&path).unwrap();

        let doc = Document::open(&path).unwrap();
        assert_eq!(doc.page_count(), 3);
        // /Kids was reversed, so the page tree order is 120, 110, 100 — and it is
        // the opposite of the object-number order (which is 100, 110, 120).
        let mut by_object_number = ids.clone();
        by_object_number.sort();
        assert_ne!(
            by_object_number, ids,
            "the fixture must have page order opposite to object order"
        );
        assert_eq!(doc.media_box(1).unwrap().width(), 120.0);
        assert_eq!(doc.media_box(2).unwrap().width(), 110.0);
        assert_eq!(doc.media_box(3).unwrap().width(), 100.0);
        let _ = std::fs::remove_file(&path);
    }
}

fn array_to_box(obj: &Object) -> Option<Box4> {
    let arr = obj.as_array().ok()?;
    if arr.len() != 4 {
        return None;
    }
    let mut v = [0f64; 4];
    for (i, o) in arr.iter().enumerate() {
        v[i] = match o {
            Object::Integer(i) => *i as f64,
            Object::Real(r) => *r as f64,
            _ => return None,
        };
    }
    Some(Box4::new(v[0], v[1], v[2], v[3]))
}

fn box_to_array(b: Box4, hires: bool) -> Object {
    let num = |v: f64| -> Object {
        if hires {
            Object::Real(v as f32)
        } else {
            Object::Integer(v.round() as i64)
        }
    };
    Object::Array(vec![num(b.x0), num(b.y0), num(b.x1), num(b.y1)])
}

/// Only used to keep `Dictionary` in scope for the doc comment above.
#[allow(dead_code)]
fn _assert_dict_used(_: &Dictionary) {}
