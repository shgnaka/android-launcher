use crate::model::AppEntry;
use unicode_normalization::UnicodeNormalization;

/// Replace or compose this interface to add aliases, tokenization or ranking.
pub trait AppSearch: Send + Sync {
    fn search<'a>(&self, query: &str, apps: &'a [AppEntry]) -> Vec<&'a AppEntry>;
}

#[derive(Default)]
pub struct ExactSearch;

/// Small offline reading dictionary; unknown pronunciations can be supplied by the user.
#[derive(Default)]
pub struct ReadingSearch {
    readings: std::collections::BTreeMap<String, String>,
    index: std::sync::Mutex<Option<ReadingIndex>>,
}
struct ReadingIndex {
    fingerprint: u64,
    keys: Vec<(String, Option<String>)>,
}
impl ReadingSearch {
    pub fn with_readings(readings: std::collections::BTreeMap<String, String>) -> Self {
        Self {
            readings,
            index: Default::default(),
        }
    }
}
impl AppSearch for ReadingSearch {
    fn search<'a>(&self, query: &str, apps: &'a [AppEntry]) -> Vec<&'a AppEntry> {
        let literal = normalize(query);
        if literal.is_empty() {
            return apps.iter().collect();
        }
        let reading = reading_key(query);
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        for app in apps {
            app.id.hash(&mut hash);
            app.label.hash(&mut hash);
        }
        let fingerprint = hash.finish();
        let mut index = self.index.lock().unwrap_or_else(|error| error.into_inner());
        if index
            .as_ref()
            .is_none_or(|index| index.fingerprint != fingerprint)
        {
            *index = Some(ReadingIndex {
                fingerprint,
                keys: apps
                    .iter()
                    .map(|app| {
                        (
                            reading_key(&app.label),
                            self.readings.get(&app.id).map(|value| reading_key(value)),
                        )
                    })
                    .collect(),
            });
        }
        apps.iter()
            .zip(&index.as_ref().unwrap().keys)
            .filter(|(_, key)| key.0 == reading || key.1.as_ref() == Some(&reading))
            .map(|(app, _)| app)
            .collect()
    }
}
fn brand_reading(name: &str) -> Option<&'static str> {
    Some(match name {
        "line" => "らいん",
        "chrome" | "google chrome" => "くろーむ",
        "youtube" => "ゆーちゅーぶ",
        "gmail" => "じーめーる",
        "google" => "ぐーぐる",
        "google maps" => "ぐーぐるまっぷ",
        "spotify" => "すぽてぃふぁい",
        "firefox" => "ふぁいあふぉっくす",
        "slack" => "すらっく",
        "discord" => "でぃすこーど",
        "amazon" => "あまぞん",
        "kindle" => "きんどる",
        "twitter" => "ついったー",
        "instagram" => "いんすたぐらむ",
        "facebook" => "ふぇいすぶっく",
        "github" => "ぎっとはぶ",
        "chatgpt" => "ちゃっとじーぴーてぃー",
        "tiktok" => "てぃっくとっく",
        _ => return None,
    })
}
fn reading_key(value: &str) -> String {
    let normalized = normalize(value);
    let value = brand_reading(&normalized).unwrap_or(&normalized);
    let kana: String = value
        .nfkc()
        .map(|c| {
            if ('ァ'..='ヶ').contains(&c) {
                char::from_u32(c as u32 - 0x60).unwrap()
            } else {
                c
            }
        })
        .collect();
    let chars: Vec<char> = kana.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == 'ー' || c.is_whitespace() || matches!(c, '’' | '\'') {
            i += 1;
            continue;
        }
        if c == 'っ' {
            if let Some(&next) = chars.get(i + 1) {
                let part = kana_piece(&next.to_string()).unwrap_or("");
                if let Some(first) = part.chars().next() {
                    if !"aeioun".contains(first) {
                        out.push(first);
                    }
                }
            }
            i += 1;
            continue;
        }
        if let Some(&next) = chars.get(i + 1) {
            let pair: String = [c, next].into_iter().collect();
            if let Some(part) = kana_piece(&pair) {
                out.push_str(part);
                i += 2;
                continue;
            }
        }
        if let Some(part) = kana_piece(&c.to_string()) {
            out.push_str(part);
        } else {
            out.push(c);
        }
        i += 1;
    }
    for (from, to) in [
        ("cchi", "tti"),
        ("tchi", "tti"),
        ("sha", "sya"),
        ("shu", "syu"),
        ("sho", "syo"),
        ("cha", "tya"),
        ("chu", "tyu"),
        ("cho", "tyo"),
        ("ja", "zya"),
        ("ju", "zyu"),
        ("jo", "zyo"),
        ("shi", "si"),
        ("chi", "ti"),
        ("tsu", "tu"),
        ("fu", "hu"),
        ("ji", "zi"),
        ("ca", "ka"),
        ("co", "ko"),
        ("cu", "ku"),
    ] {
        out = out.replace(from, to);
    }
    let out = out.replace("ou", "o");
    let mut key = String::new();
    let mut previous = None;
    for c in out
        .nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
    {
        if !(previous == Some(c) && "aeiou".contains(c)) {
            key.push(c);
        }
        previous = Some(c);
    }
    key
}
fn kana_piece(value: &str) -> Option<&'static str> {
    const TABLE:&str="あ:a い:i う:u え:e お:o ぁ:a ぃ:i ぅ:u ぇ:e ぉ:o か:ka き:ki く:ku け:ke こ:ko が:ga ぎ:gi ぐ:gu げ:ge ご:go さ:sa し:shi す:su せ:se そ:so ざ:za じ:ji ず:zu ぜ:ze ぞ:zo た:ta ち:chi つ:tsu て:te と:to だ:da ぢ:ji づ:zu で:de ど:do な:na に:ni ぬ:nu ね:ne の:no は:ha ひ:hi ふ:fu へ:he ほ:ho ば:ba び:bi ぶ:bu べ:be ぼ:bo ぱ:pa ぴ:pi ぷ:pu ぺ:pe ぽ:po ま:ma み:mi む:mu め:me も:mo や:ya ゆ:yu よ:yo ゃ:ya ゅ:yu ょ:yo ら:ra り:ri る:ru れ:re ろ:ro わ:wa ゐ:wi ゑ:we を:wo ん:n ゔ:vu きゃ:kya きゅ:kyu きょ:kyo ぎゃ:gya ぎゅ:gyu ぎょ:gyo しゃ:sha しゅ:shu しょ:sho じゃ:ja じゅ:ju じょ:jo ちゃ:cha ちゅ:chu ちょ:cho にゃ:nya にゅ:nyu にょ:nyo ひゃ:hya ひゅ:hyu ひょ:hyo びゃ:bya びゅ:byu びょ:byo ぴゃ:pya ぴゅ:pyu ぴょ:pyo みゃ:mya みゅ:myu みょ:myo りゃ:rya りゅ:ryu りょ:ryo ふぁ:fa ふぃ:fi ふぇ:fe ふぉ:fo てぃ:ti でぃ:di とぅ:tu どぅ:du うぃ:wi うぇ:we うぉ:wo ゔぁ:va ゔぃ:vi ゔぇ:ve ゔぉ:vo しぇ:she ちぇ:che じぇ:je つぁ:tsa つぃ:tsi つぇ:tse つぉ:tso";
    static MAP: std::sync::OnceLock<std::collections::HashMap<&'static str, &'static str>> =
        std::sync::OnceLock::new();
    MAP.get_or_init(|| {
        TABLE
            .split_whitespace()
            .map(|pair| pair.split_once(':').unwrap())
            .collect()
    })
    .get(value)
    .copied()
}

fn normalize(value: &str) -> String {
    value.trim().nfkc().flat_map(char::to_lowercase).collect()
}

impl AppSearch for ExactSearch {
    fn search<'a>(&self, query: &str, apps: &'a [AppEntry]) -> Vec<&'a AppEntry> {
        let query = normalize(query);
        apps.iter()
            .filter(|app| query.is_empty() || normalize(&app.label) == query)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn apps() -> Vec<AppEntry> {
        [("one", "Camera"), ("two", "カメラ"), ("three", "Camera")]
            .into_iter()
            .map(|(id, label)| AppEntry {
                id: id.into(),
                label: label.into(),
            })
            .collect()
    }
    #[test]
    fn exact_match_keeps_distinct_activities_and_rejects_substrings() {
        let apps = apps();
        assert_eq!(ExactSearch.search("Camera", &apps).len(), 2);
        assert!(ExactSearch.search("Cam", &apps).is_empty());
        assert_eq!(ExactSearch.search("", &apps).len(), 3);
    }
    #[test]
    fn normalizes_width_case_whitespace_and_kana() {
        let apps = apps();
        assert_eq!(ExactSearch.search(" ＣＡＭＥＲＡ ", &apps).len(), 2);
        assert_eq!(ExactSearch.search("ｶﾒﾗ", &apps)[0].id, "two");
    }
}
