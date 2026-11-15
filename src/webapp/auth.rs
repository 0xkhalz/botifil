use WebAppAuthError as E;
use std::fmt::Write as _;
use teloxide::types::UserId;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WebAppAuthError {
    #[error("initData is empty")]
    EmptyInitData,
    #[error("missing hash in initData")]
    MissingHash,
    #[error("invalid hash or percent encoding")]
    InvalidHashEncoding,
    #[error("initData signature mismatch")]
    SignatureMismatch,
    #[error("missing auth_date in initData")]
    MissingAuthDate,
    #[error("invalid auth_date in initData")]
    InvalidAuthDate,
    #[error("initData auth_date expired or in the future")]
    ExpiredInitData,
    #[error("missing user in initData")]
    MissingUser,
    #[error("invalid user id in initData")]
    InvalidUserId,
}

#[rustfmt::skip]
const SHA256_K: [u32; 64] = [
    0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5, 0x3956_c25b, 0x59f1_11f1, 0x923f_82a4, 0xab1c_5ed5, 0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3, 0x72be_5d74, 0x80de_b1fe, 0x9bdc_06a7, 0xc19b_f174,
    0xe49b_69c1, 0xefbe_4786, 0x0fc1_9dc6, 0x240c_a1cc, 0x2de9_2c6f, 0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da, 0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7, 0xc6e0_0bf3, 0xd5a7_9147, 0x06ca_6351, 0x1429_2967,
    0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc, 0x5338_0d13, 0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85, 0xa2bf_e8a1, 0xa81a_664b, 0xc24b_8b70, 0xc76c_51a3, 0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070,
    0x19a4_c116, 0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5, 0x391c_0cb3, 0x4ed8_aa4a, 0x5b9c_ca4f, 0x682e_6ff3, 0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208, 0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7, 0xc671_78f2,
];

fn sha256(data: &[u8]) -> [u8; 32] {
    let (rr, add) = (u32::rotate_right, u32::wrapping_add);
    let mut buf = [data, &[0x80]].concat();
    buf.resize((buf.len() + 8).div_ceil(64) * 64 - 8, 0);
    buf.extend_from_slice(
        &(u64::try_from(data.len()).unwrap_or(0).saturating_mul(8)).to_be_bytes(),
    );
    #[rustfmt::skip]
    let mut state: [u32; 8] = [0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a, 0x510e_527f, 0x9b05_688c, 0x1f83_d9ab, 0x5be0_cd19];
    for block in buf.as_chunks::<64>().0 {
        let mut words = [0_u32; 64];
        for (slot, chunk) in words.iter_mut().zip(block.as_chunks::<4>().0) {
            *slot = u32::from_be_bytes(*chunk);
        }
        for i in 16..64 {
            let s0 = rr(words[i - 15], 7) ^ rr(words[i - 15], 18) ^ (words[i - 15] >> 3);
            let s1 = rr(words[i - 2], 17) ^ rr(words[i - 2], 19) ^ (words[i - 2] >> 10);
            words[i] = add(add(words[i - 16], words[i - 7]), add(s0, s1));
        }
        let mut s = state;
        for (&rk, &wi) in SHA256_K.iter().zip(&words) {
            let sig1 = rr(s[4], 6) ^ rr(s[4], 11) ^ rr(s[4], 25);
            let ch = (s[4] & s[5]) ^ ((!s[4]) & s[6]);
            let t1 = add(add(s[7], sig1), add(ch, add(rk, wi)));
            let sig0 = rr(s[0], 2) ^ rr(s[0], 13) ^ rr(s[0], 22);
            let maj = (s[0] & s[1]) ^ (s[0] & s[2]) ^ (s[1] & s[2]);
            s.rotate_right(1);
            s[4] = add(s[4], t1);
            s[0] = add(t1, add(sig0, maj));
        }
        for (dst, word) in state.iter_mut().zip(s) {
            *dst = add(*dst, word);
        }
    }
    let mut out = [0_u8; 32];
    for (dst, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(state) {
        *dst = word.to_be_bytes();
    }
    out
}

fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut pad = [0_u8; 64];
    let hk = (key.len() > 64).then(|| sha256(key));
    let src = hk.as_ref().map_or(key, <[u8; 32]>::as_slice);
    pad[..src.len()].copy_from_slice(src);
    let inner = [&pad.map(|b| b ^ 0x36)[..], msg].concat();
    sha256(&[&pad.map(|b| b ^ 0x5c)[..], &sha256(&inner)].concat())
}

fn ensure(ok: bool, err: WebAppAuthError) -> Result<(), WebAppAuthError> {
    if ok { Ok(()) } else { Err(err) }
}

fn hex_pair(pair: &[u8]) -> Result<u8, WebAppAuthError> {
    let &[hi, lo] = pair else {
        return Err(E::InvalidHashEncoding);
    };
    let (Some(h), Some(l)) = (char::from(hi).to_digit(16), char::from(lo).to_digit(16)) else {
        return Err(E::InvalidHashEncoding);
    };
    u8::try_from((h << 4) | l).map_err(|_| E::InvalidHashEncoding)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn check_str<'a>(pairs: impl Iterator<Item = (&'a str, &'a str)>) -> String {
    let mut vec: Vec<_> = pairs.collect();
    vec.sort_by_key(|p| p.0);
    let list: Vec<_> = vec.into_iter().map(|(k, v)| format!("{k}={v}")).collect();
    list.join("\n")
}

/// Builds a signed Telegram `initData` query string for tests.
pub fn sign_init_data_for_test(pairs: &[(&str, &str)], bot_token: &str) -> String {
    let check = check_str(pairs.iter().copied().filter(|(k, _)| *k != "hash"));
    let sec = hmac_sha256(b"WebAppData", bot_token.as_bytes());
    let mac = hmac_sha256(&sec, check.as_bytes());
    let mut ser = url::form_urlencoded::Serializer::new(String::new());
    for &(key, val) in pairs.iter().filter(|(k, _)| *k != "hash") {
        ser.append_pair(key, val);
    }
    ser.append_pair("hash", &hex_encode(&mac));
    ser.finish()
}

fn extract_user_id(json: &str) -> Result<u64, WebAppAuthError> {
    let Some((&b'{', mut rest)) = json.as_bytes().trim_ascii_start().split_first() else {
        return Err(E::InvalidUserId);
    };
    let (mut depth, mut key) = (1_usize, true);
    while depth > 0
        && let Some((&ch, next)) = rest.trim_ascii_start().split_first()
    {
        rest = next;
        match ch {
            b'"' => {
                let mut i = 0;
                while i < rest.len() && rest[i] != b'"' {
                    i += if rest[i] == b'\\' { 2 } else { 1 };
                }
                ensure(i < rest.len(), E::InvalidUserId)?;
                let name = &rest[..i];
                rest = &rest[i + 1..];
                let colon = rest.trim_ascii_start().strip_prefix(b":");
                if depth == 1
                    && key
                    && let Some(after) = colon
                {
                    if name == b"id" {
                        let val = after.trim_ascii_start();
                        let n = val.iter().take_while(|b| b.is_ascii_digit()).count();
                        let ok = matches!(val[n..].trim_ascii_start().first(), Some(b',' | b'}'));
                        ensure(ok, E::InvalidUserId)?;
                        let raw = std::str::from_utf8(&val[..n]).map_err(|_| E::InvalidUserId)?;
                        return raw.parse().map_err(|_| E::InvalidUserId);
                    }
                    key = false;
                }
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth -= 1,
            b',' if depth == 1 => key = true,
            _ => {}
        }
    }
    Err(E::InvalidUserId)
}

/// Verifies Telegram Mini App `initData` HMAC-SHA256 signature, freshness, and user ID.
///
/// # Errors
/// Returns [`WebAppAuthError`] if `init_data` is empty, malformed, unsigned, expired, or lacks a numeric user ID.
pub fn verify_telegram_init_data(
    init_data: &str,
    bot_token: &str,
    now_unix: u64,
    challenge_ttl_secs: u64,
) -> Result<UserId, WebAppAuthError> {
    ensure(!init_data.is_empty(), E::EmptyInitData)?;
    let (mut hash_hex, mut pairs) = (None, Vec::new());
    for (k, v) in url::form_urlencoded::parse(init_data.as_bytes()) {
        if k == "hash" {
            ensure(
                hash_hex.replace(v.into_owned()).is_none(),
                E::InvalidHashEncoding,
            )?;
        } else {
            pairs.push((k.into_owned(), v.into_owned()));
        }
    }
    let hs = hash_hex.ok_or(E::MissingHash)?;
    ensure(hs.len() == 64, E::InvalidHashEncoding)?;
    let mut expected = [0_u8; 32];
    for (dst, chunk) in expected.iter_mut().zip(hs.as_bytes().as_chunks::<2>().0) {
        *dst = hex_pair(chunk)?;
    }
    let check = check_str(pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    let sec = hmac_sha256(b"WebAppData", bot_token.as_bytes());
    let got = hmac_sha256(&sec, check.as_bytes());
    let diff = got.iter().zip(expected).fold(0_u8, |a, (x, y)| a | (x ^ y));
    ensure(std::hint::black_box(diff) == 0, E::SignatureMismatch)?;
    let field = |n: &str| pairs.iter().find(|p| p.0 == n).map(|p| p.1.as_str());
    let auth = field("auth_date").ok_or(E::MissingAuthDate)?;
    ensure(auth.bytes().all(|b| b.is_ascii_digit()), E::InvalidAuthDate)?;
    let ts: u64 = auth.parse().map_err(|_| E::InvalidAuthDate)?;
    let fresh =
        ts <= now_unix.saturating_add(60) && now_unix.saturating_sub(ts) <= challenge_ttl_secs;
    ensure(fresh, E::ExpiredInitData)?;
    extract_user_id(field("user").ok_or(E::MissingUser)?).map(UserId)
}

#[cfg(test)]
mod tests {
    use super::*;
    const TOK: &str = "987654:test-secret-token";

    #[test]
    fn sha256_and_hmac_rfc4231_vectors() {
        #[rustfmt::skip]
        let vecs = [
            (sha256(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
            (sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
            (hmac_sha256(&[0x0b; 20], b"Hi There"), "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"),
            (hmac_sha256(b"Jefe", b"what do ya want for nothing?"), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"),
            (hmac_sha256(&[0xaa; 20], &[0xdd; 50]), "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"),
            (hmac_sha256(&[0xaa; 131], b"Test Using Larger Than Block-Size Key - Hash Key First"), "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"),
        ];
        for (digest, want) in vecs {
            assert_eq!(hex_encode(&digest), want);
        }
    }

    #[test]
    fn verifies_init_data_and_checks_errors() {
        let with_user = |u| sign_init_data_for_test(&[("auth_date", "1000"), ("user", u)], TOK);
        #[rustfmt::skip]
        let init = sign_init_data_for_test(&[("query_id", "AAHdF"), ("user", r#"{"id":4242,"first_name":"Alice"}"#), ("auth_date", "1000")], TOK);
        let mut bad_hash = init.clone();
        let last = if bad_hash.ends_with('0') { "1" } else { "0" };
        bad_hash.replace_range(bad_hash.len() - 1.., last);
        let dup_hash = format!("{init}&hash=0000");
        #[rustfmt::skip]
        let cases: &[(&str, &str, u64, Result<u64, E>)] = &[
            (&init, TOK, 1300, Ok(4242)),
            (&init, TOK, 940, Ok(4242)),
            (&init, TOK, 1301, Err(E::ExpiredInitData)),
            (&init, TOK, 939, Err(E::ExpiredInitData)),
            (&init, "wrong", 1000, Err(E::SignatureMismatch)),
            (&bad_hash, TOK, 1000, Err(E::SignatureMismatch)),
            (&init.replace("AAHdF", "AAHdG"), TOK, 1000, Err(E::SignatureMismatch)),
            ("", TOK, 1000, Err(E::EmptyInitData)),
            ("auth_date=1000", TOK, 1000, Err(E::MissingHash)),
            ("auth_date=1000&hash=abc", TOK, 1000, Err(E::InvalidHashEncoding)),
            (&format!("auth_date=1000&hash={}", "zz".repeat(32)), TOK, 1000, Err(E::InvalidHashEncoding)),
            (&dup_hash, TOK, 1000, Err(E::InvalidHashEncoding)),
            (&with_user(r#"{"first_name":"\"id\":999, \\","meta":{"id":777},"id":555}"#), TOK, 1000, Ok(555)),
            (&with_user(r#"{"first_name":"\"id\":999"}"#), TOK, 1000, Err(E::InvalidUserId)),
            (&with_user(r#"{"meta":{"id":777}}"#), TOK, 1000, Err(E::InvalidUserId)),
            (&with_user(r#"{"id":"555"}"#), TOK, 1000, Err(E::InvalidUserId)),
            (&sign_init_data_for_test(&[("user", r#"{"id":555}"#)], TOK), TOK, 1000, Err(E::MissingAuthDate)),
            (&sign_init_data_for_test(&[("auth_date", "1000")], TOK), TOK, 1000, Err(E::MissingUser)),
            (&sign_init_data_for_test(&[("auth_date", "+1000"), ("user", r#"{"id":555}"#)], TOK), TOK, 1000, Err(E::InvalidAuthDate)),
        ];
        let check = |r, t, n| verify_telegram_init_data(r, t, n, 300).map(|u| u.0);
        for &(raw, tok, now, ref want) in cases {
            assert_eq!(check(raw, tok, now), *want);
        }
    }
}
