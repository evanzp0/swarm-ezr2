#![allow(clippy::cognitive_complexity, clippy::too_many_lines)]
#![allow(missing_docs)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use std::collections::BTreeMap;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut out: Option<PathBuf> = None;
    let mut piece_len: usize = 16 * 1024;
    let mut announces: Vec<String> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    let mut name_flag: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => out = args.next().map(PathBuf::from),
            "--name" => name_flag = args.next(),
            "--piece-len" => {
                piece_len = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(16 * 1024);
            }
            "--announce" => {
                if let Some(u) = args.next() {
                    announces.push(u);
                }
            }
            _ => files.push(PathBuf::from(a)),
        }
    }
    let Some(out) = out else {
        eprintln!(
            "用法：ezr-mktorrent --out <x.torrent> [--piece-len N] [--announce url] <file>..."
        );
        std::process::exit(2);
    };
    if files.is_empty() {
        eprintln!("至少一个输入文件");
        std::process::exit(2);
    }
    let torrent = build(&files, piece_len, &announces, name_flag);
    if let Some(dir) = out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&out, torrent).expect("写种子失败");
    println!("{}", out.display());
}

fn build(
    files: &[PathBuf],
    piece_len: usize,
    announces: &[String],
    name_flag: Option<String>,
) -> Vec<u8> {
    let single = files.len() == 1;
    let root_name = if single {
        files[0]
            .file_name()
            .map_or_else(|| "root".into(), |n| n.to_string_lossy().into_owned())
    } else {
        name_flag.unwrap_or_else(|| "root".into())
    };

    let mut data = Vec::new();
    let mut lengths: Vec<u64> = Vec::new();
    for f in files {
        let bytes = std::fs::read(f).unwrap_or_else(|e| panic!("读 {} 失败：{e}", f.display()));
        lengths.push(bytes.len() as u64);
        data.extend_from_slice(&bytes);
    }
    let mut pieces = Vec::new();
    for chunk in data.chunks(piece_len.max(1)) {
        use sha1::{Digest, Sha1};
        let mut h = Sha1::new();
        h.update(chunk);
        pieces.extend_from_slice(&h.finalize());
    }

    let mut info = BTreeMap::new();
    if single {
        info.insert(b"length".to_vec(), be_int(lengths[0] as i64));
    } else {
        let fl: Vec<bencode::BValue> = files
            .iter()
            .zip(&lengths)
            .map(|(f, len)| {
                let segs: Vec<bencode::BValue> = [
                    root_name.as_str(),
                    &f.file_name()
                        .map_or_default(|n| n.to_string_lossy().into_owned()),
                ]
                .iter()
                .filter(|s| !s.is_empty())
                .map(|s| bencode::BValue::Bytes(s.as_bytes().to_vec()))
                .collect();
                let mut m = BTreeMap::new();
                m.insert(b"length".to_vec(), be_int(*len as i64));
                m.insert(b"path".to_vec(), bencode::BValue::List(segs));
                bencode::BValue::Dict(m)
            })
            .collect();
        info.insert(b"files".to_vec(), bencode::BValue::List(fl));
    }
    info.insert(
        b"name".to_vec(),
        bencode::BValue::Bytes(root_name.into_bytes()),
    );
    info.insert(b"piece length".to_vec(), be_int(piece_len as i64));
    info.insert(b"pieces".to_vec(), bencode::BValue::Bytes(pieces));

    let mut root = BTreeMap::new();
    if let Some(a) = announces.first() {
        root.insert(
            b"announce".to_vec(),
            bencode::BValue::Bytes(a.clone().into_bytes()),
        );
    }
    if announces.len() > 1 {
        let tiers: Vec<bencode::BValue> = announces
            .iter()
            .map(|u| bencode::BValue::List(vec![bencode::BValue::Bytes(u.clone().into_bytes())]))
            .collect();
        root.insert(b"announce-list".to_vec(), bencode::BValue::List(tiers));
    }
    root.insert(b"info".to_vec(), bencode::BValue::Dict(info));
    bencode::encode(&bencode::BValue::Dict(root))
}

const fn be_int(v: i64) -> bencode::BValue {
    bencode::BValue::Int(v)
}

mod bencode {
    use std::collections::BTreeMap;

    #[derive(Clone, Debug, PartialEq)]
    pub enum BValue {
        Int(i64),
        Bytes(Vec<u8>),
        List(Vec<Self>),
        Dict(BTreeMap<Vec<u8>, Self>),
    }

    pub fn encode(v: &BValue) -> Vec<u8> {
        let mut out = Vec::new();
        encode_into(v, &mut out);
        out
    }

    fn encode_into(v: &BValue, out: &mut Vec<u8>) {
        match v {
            BValue::Int(i) => {
                out.push(b'i');
                out.extend_from_slice(i.to_string().as_bytes());
                out.push(b'e');
            }
            BValue::Bytes(b) => {
                out.extend_from_slice(b.len().to_string().as_bytes());
                out.push(b':');
                out.extend_from_slice(b);
            }
            BValue::List(l) => {
                out.push(b'l');
                for item in l {
                    encode_into(item, out);
                }
                out.push(b'e');
            }
            BValue::Dict(m) => {
                out.push(b'd');
                for (k, val) in m {
                    encode_into(&BValue::Bytes(k.clone()), out);
                    encode_into(val, out);
                }
                out.push(b'e');
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torrent_shape_contract() {
        let dir = std::env::temp_dir().join(format!(
            "ezr-mkt-{}-{}",
            std::process::id(),
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let f1 = dir.join("in-a.bin");
        let f2 = dir.join("in-b.bin");
        std::fs::write(&f1, vec![1u8; 50_000]).unwrap();
        std::fs::write(&f2, vec![2u8; 30_000]).unwrap();
        let bytes = build(
            &[f1, f2],
            16 * 1024,
            &["udp://127.0.0.1:1/ann".into()],
            Some("root".into()),
        );
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("4:name4:root"), "根名段: {text}");
        assert!(text.contains("5:files"), "多文件 files 段");
        assert!(text.contains("12:piece lengthi16384e"), "piece length 段");
        assert!(text.contains("6:pieces"), "pieces 段");
        assert!(text.contains("8:announce"), "announce 段");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
