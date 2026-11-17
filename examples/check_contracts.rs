use std::{
    collections::BTreeMap,
    io::Write,
    process::{Command, Stdio},
};
fn hash(bytes: &[u8]) -> String {
    let mut p = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    p.stdin.take().unwrap().write_all(bytes).unwrap();
    let o = p.wait_with_output().unwrap();
    assert!(o.status.success());
    String::from_utf8(o.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .into()
}
fn main() {
    let files: BTreeMap<String, String> =
        serde_json::from_slice(&std::fs::read("tests/frozen-nov16.sha256.json").unwrap()).unwrap();
    for (p, expected) in files {
        assert_eq!(
            hash(&std::fs::read(&p).unwrap()),
            expected,
            "frozen file modified: {p}"
        );
    }
    let legacy: BTreeMap<String, String> =
        serde_json::from_slice(&std::fs::read("tests/legacy-tests.sha256.json").unwrap()).unwrap();
    for (key, expected) in legacy {
        let (path, _) = key.rsplit_once(':').unwrap();
        let src = std::fs::read_to_string(path).unwrap();
        let module = src.find("mod tests {").unwrap();
        let start = src[..module].rfind("#[cfg").unwrap();
        let end = module + src[module..].find("\n}").unwrap() + 2;
        assert_eq!(
            hash(src[start..end].as_bytes()),
            expected,
            "legacy tests modified: {path}"
        );
    }
    println!("Frozen acceptance tests and existing test modules are unchanged.");
}
