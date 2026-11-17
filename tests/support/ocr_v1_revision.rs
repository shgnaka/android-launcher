//! Reviewable correction to line-center tapping. Does not change the frozen v1 helper.
use anyhow::{ensure, Result};
use std::collections::BTreeMap;
fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_uppercase)
        .collect()
}
pub fn control_bounds(tsv: &str, target: &str) -> Result<[i32; 4]> {
    let needle = norm(target);
    ensure!(!needle.is_empty(), "empty control selector");
    let mut lines: BTreeMap<String, Vec<(String, [i32; 4])>> = BTreeMap::new();
    for row in tsv.lines() {
        let c: Vec<_> = row.split('\t').collect();
        if c.len() < 12 || c[0] != "5" || c[11].trim().is_empty() {
            continue;
        }
        let x: i32 = c[6].parse()?;
        let y: i32 = c[7].parse()?;
        let w: i32 = c[8].parse()?;
        let h: i32 = c[9].parse()?;
        ensure!(w > 0 && h > 0, "invalid OCR box");
        lines
            .entry(format!("{}:{}:{}:{}", c[1], c[2], c[3], c[4]))
            .or_default()
            .push((norm(c[11]), [x, y, x + w, y + h]));
    }
    let mut found = vec![];
    for words in lines.values() {
        let text = words.iter().map(|(s, _)| s.as_str()).collect::<String>();
        for (start, _) in text.match_indices(&needle) {
            let end = start + needle.len();
            let mut offset = 0;
            let mut boxes = vec![];
            for (word, b) in words {
                let stop = offset + word.len();
                if offset < end && stop > start {
                    ensure!(
                        offset >= start && stop <= end,
                        "selector shares an OCR token with unrelated text; cannot locate safely"
                    );
                    boxes.push(*b);
                }
                offset = stop;
            }
            let first = boxes.first().copied().unwrap();
            found.push(boxes.iter().fold(first, |a, b| {
                [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[2].max(b[2]),
                    a[3].max(b[3]),
                ]
            }));
        }
    }
    ensure!(
        found.len() == 1,
        "control {target:?} must have one unambiguous word box, found {}",
        found.len()
    );
    Ok(found[0])
}
