//! The view options an embedding page may set on the files it opens (`?host=parent`): `mode`,
//! `zoom` and `page`, from the query string or a `pdfcraft:open` message.
//!
//! Both come from another page, so only these keys are heard and each value is checked here
//! before it reaches [`pdfcraft_ui_egui::PdfCraftApp::set_option`]. A value that doesn't fit is
//! dropped (the option keeps its default); nothing an embedder sends can fail the open.

/// The keys in the order they are applied: `page` last, so a fit zoom doesn't move off it.
pub const KEYS: [&str; 3] = ["mode", "zoom", "page"];

const MODES: [&str; 5] = ["all", "read", "edit", "convert", "sign"];
/// The zoom range the canvas allows, in percent.
const ZOOM_PERCENT: std::ops::RangeInclusive<f32> = 8.0..=6400.0;
/// Longer values are not options anyone means.
const MAX_LEN: usize = 32;

/// A raw option value: a string from the query or a message, or a number from a message.
#[derive(Clone, Copy, Debug)]
pub enum Raw<'a> {
    Text(&'a str),
    Number(f64),
}

/// The value `set_option` gets for `key`, or `None` when the key isn't one of [`KEYS`] or the
/// value doesn't fit it.
pub fn normalize(key: &str, raw: Raw<'_>) -> Option<String> {
    let text = match raw {
        Raw::Text(t) if t.len() <= MAX_LEN => t.trim().to_string(),
        Raw::Text(_) => return None,
        Raw::Number(n) => number_text(n)?,
    };
    match key {
        "mode" => MODES.iter().find(|m| text.eq_ignore_ascii_case(m)).map(|m| (*m).to_string()),
        "zoom" => {
            if text.eq_ignore_ascii_case("fit-width") || text.eq_ignore_ascii_case("fit-page") {
                return Some(text.to_ascii_lowercase());
            }
            let percent = text.strip_suffix('%').unwrap_or(&text).trim().parse::<f32>().ok()?;
            ZOOM_PERCENT.contains(&percent).then(|| percent.to_string())
        }
        // 1-based; past the last page lands on the last page.
        "page" => text.parse::<u32>().ok().filter(|&p| p >= 1).map(|p| p.to_string()),
        _ => None,
    }
}

/// A finite number as text, without a trailing `.0` for whole numbers.
fn number_text(n: f64) -> Option<String> {
    if !n.is_finite() {
        return None;
    }
    if n.fract() == 0.0 && n.abs() <= f64::from(u32::MAX) {
        // Whole and in range, so the conversion is exact.
        return Some(format!("{n:.0}"));
    }
    Some(n.to_string())
}

/// The options to apply, in [`KEYS`] order: each key from `message` when it has a valid value
/// there, else from `query`.
pub fn merge(query: &[(&'static str, String)], message: &[(&'static str, String)]) -> Vec<(String, String)> {
    KEYS.iter()
        .filter_map(|key| {
            let pick = |list: &[(&'static str, String)]| list.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
            pick(message).or_else(|| pick(query)).map(|v| ((*key).to_string(), v))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_keys_and_values_pass() {
        assert_eq!(normalize("mode", Raw::Text(" Read ")).as_deref(), Some("read"));
        assert_eq!(normalize("mode", Raw::Text("admin")), None);
        assert_eq!(normalize("zoom", Raw::Text("FIT-WIDTH")).as_deref(), Some("fit-width"));
        assert_eq!(normalize("zoom", Raw::Text("fit-page")).as_deref(), Some("fit-page"));
        assert_eq!(normalize("zoom", Raw::Text("150%")).as_deref(), Some("150"));
        assert_eq!(normalize("zoom", Raw::Number(75.5)).as_deref(), Some("75.5"));
        assert_eq!(normalize("page", Raw::Text("3")).as_deref(), Some("3"));
        assert_eq!(normalize("page", Raw::Number(12.0)).as_deref(), Some("12"));
        for key in ["panel", "dialog", "tool", "home", "", "Page"] {
            assert_eq!(normalize(key, Raw::Text("1")), None, "{key}");
        }
    }

    #[test]
    fn hostile_values_are_dropped() {
        for zoom in ["0", "7", "6401", "1e9", "NaN", "inf", "-100", "%", "fit", "100%%"] {
            assert_eq!(normalize("zoom", Raw::Text(zoom)), None, "{zoom}");
        }
        for page in ["0", "-1", "1.5", "4294967296", "99999999999999999999", "", "1 2", "+"] {
            assert_eq!(normalize("page", Raw::Text(page)), None, "{page}");
        }
        for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, -3.0, 2.5, 1e300, f64::MAX] {
            assert_eq!(normalize("page", Raw::Number(n)), None, "{n}");
        }
        assert_eq!(normalize("zoom", Raw::Number(f64::NAN)), None);
        assert_eq!(normalize("zoom", Raw::Number(1e300)), None);
        assert_eq!(normalize("mode", Raw::Text(&"read".repeat(1000))), None);
        // Multi-byte text is measured in bytes and never sliced.
        assert_eq!(normalize("mode", Raw::Text(&"é".repeat(20))), None);
    }

    #[test]
    fn message_fields_override_query_params_key_by_key() {
        let query = [("mode", "edit".to_string()), ("zoom", "200".to_string())];
        let message = [("zoom", "fit-page".to_string()), ("page", "4".to_string())];
        let merged = merge(&query, &message);
        let expected: Vec<(String, String)> =
            [("mode", "edit"), ("zoom", "fit-page"), ("page", "4")].iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect();
        assert_eq!(merged, expected);
        assert!(merge(&[], &[]).is_empty());
    }
}
