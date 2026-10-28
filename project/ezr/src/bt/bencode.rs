use std::collections::BTreeMap;

use super::BtError;

#[derive(Clone, Debug, PartialEq)]
pub enum BValue {
    Int(i64),
    Bytes(Vec<u8>),
    List(Vec<BValue>),
    Dict(BTreeMap<Vec<u8>, BValue>),
}

impl BValue {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&BValue> {
        match self {
            BValue::Dict(m) => m.get(key.as_bytes()),
            _ => None,
        }
    }

    #[must_use]
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(BValue::as_str)
    }

    #[must_use]
    pub fn get_int(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(BValue::as_int)
    }

    #[must_use]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            BValue::Bytes(b) => Some(b),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        self.as_bytes().and_then(|b| std::str::from_utf8(b).ok())
    }

    #[must_use]
    pub fn as_int(&self) -> Option<i64> {
        match self {
            BValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_list(&self) -> Option<&[BValue]> {
        match self {
            BValue::List(l) => Some(l),
            _ => None,
        }
    }
}

pub fn decode(data: &[u8]) -> Result<(BValue, usize), BtError> {
    let (v, n) = decode_inner(data)?;
    Ok((v, n))
}

pub fn decode_all(data: &[u8]) -> Result<BValue, BtError> {
    let (v, n) = decode_inner(data)?;
    if n != data.len() {
        return Err(BtError::Bencode(format!(
            "尾部多余 {} 字节",
            data.len() - n
        )));
    }
    Ok(v)
}

pub fn decode_with_info_span(
    data: &[u8],
) -> Result<(BValue, Option<std::ops::Range<usize>>), BtError> {
    let (v, _) = decode_inner(data)?;
    let span = find_dict_value_span(data, 0, b"info")?;
    Ok((v, span))
}

fn find_dict_value_span(
    data: &[u8],
    pos: usize,
    key: &[u8],
) -> Result<Option<std::ops::Range<usize>>, BtError> {
    if data.get(pos) != Some(&b'd') {
        return Ok(None);
    }
    let mut p = pos + 1;
    while data.get(p) != Some(&b'e') {
        let (k, kn) = decode_inner_at(data, p)?;
        p += kn;
        let vs = p;
        let (_, vn) = decode_inner_at(data, p)?;
        p += vn;
        if k.as_bytes() == Some(key) {
            return Ok(Some(vs..p));
        }
    }
    Ok(None)
}

fn decode_inner_at(data: &[u8], pos: usize) -> Result<(BValue, usize), BtError> {
    match data.get(pos) {
        Some(b'i') => {
            let end = data[pos..]
                .iter()
                .position(|&b| b == b'e')
                .ok_or_else(|| BtError::Bencode("整数未闭合".into()))?;
            let digits = std::str::from_utf8(&data[pos + 1..pos + end])
                .map_err(|_| BtError::Bencode("整数非 UTF-8".into()))?;
            if digits.starts_with("-0") || (digits.len() > 1 && digits.starts_with('0')) {
                return Err(BtError::Bencode(format!("整数前导零：{digits}")));
            }
            let v: i64 = digits
                .parse()
                .map_err(|_| BtError::Bencode(format!("整数越界：{digits}")))?;
            Ok((BValue::Int(v), end + 1))
        }
        Some(b'l') => {
            let mut p = pos + 1;
            let mut out = Vec::new();
            while data.get(p) != Some(&b'e') {
                let (v, n) = decode_inner_at(data, p)?;
                out.push(v);
                p += n;
            }
            Ok((BValue::List(out), p + 1 - pos))
        }
        Some(b'd') => {
            let mut p = pos + 1;
            let mut map = BTreeMap::new();
            let mut prev_key: Option<Vec<u8>> = None;
            while data.get(p) != Some(&b'e') {
                let (k, kn) = decode_inner_at(data, p)?;
                let kb = k
                    .as_bytes()
                    .ok_or_else(|| BtError::Bencode("字典键非字节串".into()))?
                    .to_vec();
                if let Some(pk) = &prev_key {
                    if pk >= &kb {
                        return Err(BtError::Bencode("字典键未按字典序".into()));
                    }
                }
                prev_key = Some(kb.clone());
                p += kn;
                let (v, vn) = decode_inner_at(data, p)?;
                p += vn;
                map.insert(kb, v);
            }
            Ok((BValue::Dict(map), p + 1 - pos))
        }
        Some(c) if c.is_ascii_digit() => {
            let colon = data[pos..]
                .iter()
                .position(|&b| b == b':')
                .ok_or_else(|| BtError::Bencode("字节串缺冒号".into()))?;
            let len_str = std::str::from_utf8(&data[pos..pos + colon])
                .map_err(|_| BtError::Bencode("字节串长度非 UTF-8".into()))?;
            let len: usize = len_str
                .parse()
                .map_err(|_| BtError::Bencode(format!("字节串长度非法：{len_str}")))?;
            if len_str.len() > 1 && len_str.starts_with('0') {
                return Err(BtError::Bencode(format!("字节串长度前导零：{len_str}")));
            }
            let start = pos + colon + 1;
            let end = start
                .checked_add(len)
                .ok_or_else(|| BtError::Bencode("字节串长度越界".into()))?;
            let bytes = data
                .get(start..end)
                .ok_or_else(|| BtError::Bencode("字节串截断".into()))?;
            Ok((BValue::Bytes(bytes.to_vec()), colon + 1 + len))
        }
        _ => Err(BtError::Bencode(format!(
            "位置 {pos} 非法 bencode 起始字节 {:?}",
            data.get(pos).copied()
        ))),
    }
}

fn decode_inner(data: &[u8]) -> Result<(BValue, usize), BtError> {
    decode_inner_at(data, 0)
}

#[must_use]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_int() {
        assert!(matches!(decode_all(b"i42e"), Ok(BValue::Int(42))));
        assert!(matches!(decode_all(b"i-7e"), Ok(BValue::Int(-7))));
        assert!(matches!(decode_all(b"i0e"), Ok(BValue::Int(0))));
        assert!(decode_all(b"i04e").is_err());
        assert!(decode_all(b"i-0e").is_err());
    }

    #[test]
    fn decode_bytes_and_list() {
        let v = decode_all(b"4:spam").unwrap();
        assert_eq!(v.as_bytes(), Some(b"spam".as_slice()));
        let v = decode_all(b"l4:spami42ee").unwrap();
        let l = v.as_list().unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].as_str(), Some("spam"));
        assert_eq!(l[1].as_int(), Some(42));
    }

    #[test]
    fn decode_dict_sorted_keys() {
        let v = decode_all(b"d8:announce4:urls4:infoi1ee").unwrap();
        assert_eq!(v.get_str("announce"), Some("urls"));
        assert_eq!(v.get_int("info"), Some(1));
        assert!(decode_all(b"d4:infoi1e8:announce4:urlse").is_err());
    }

    #[test]
    fn truncated_input_rejected() {
        assert!(decode_all(b"i42").is_err());
        assert!(decode_all(b"l4:spam").is_err());
        assert!(decode_all(b"10:ab").is_err());
        assert!(decode_all(b"").is_err());
    }

    #[test]
    fn trailing_garbage_rejected() {
        assert!(decode_all(b"i42ei").is_err());
        assert!(decode_all(b"4:spamX").is_err());
    }

    #[test]
    fn encode_round_trip() {
        let src = b"d4:infod6:lengthi1024e4:name4:aaabee";
        let v = decode_all(src).unwrap();
        let enc = encode(&v);
        let v2 = decode_all(&enc).unwrap();
        assert_eq!(v, v2);
    }

    #[test]
    fn info_span_locates_exact_bytes() {
        let src = b"d8:announce4:urls4:infod6:lengthi10eee";
        let (_v, span) = decode_with_info_span(src).unwrap();
        let span = span.unwrap();
        assert_eq!(&src[span], b"d6:lengthi10ee");
    }

    #[test]
    fn info_span_none_for_non_dict() {
        let (_v, span) = decode_with_info_span(b"i42e").unwrap();
        assert!(span.is_none());
    }
}
