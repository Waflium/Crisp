pub mod argb;
pub mod basic_file_attributes;
pub mod dependency_sorter;
pub mod ease;
pub mod easing_type;
pub mod directory_lock;
pub mod hash_ops;
pub mod java_collections;
pub mod keyframe_track;
pub mod parsing;
pub mod png_info;
pub mod random;
pub mod signature_validator;
pub mod string_util;
pub mod tick_throttler;
pub mod tri_state;
pub mod uri;
pub mod valueproviders;

use crisp_core::random::RandomSource;

pub type FastIndexMap<K, V> = indexmap::IndexMap<K, V, std::hash::BuildHasherDefault<crisp_core::hash_map::FastHasher>>;
pub type FastIndexSet<K> = indexmap::IndexSet<K, std::hash::BuildHasherDefault<crisp_core::hash_map::FastHasher>>;

pub fn parse_and_validate_untrusted_uri(uri: &str) -> Result<uri::URI, uri::URISyntaxException> {
    let parsed = uri::URI::new(uri)?;
    match parsed.scheme() {
        None => Err(uri::URISyntaxException::new(uri, format!("Missing protocol in URI: {uri}"), -1)),
        Some(scheme) if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") => {
            Err(uri::URISyntaxException::new(uri, format!("Unsupported protocol in URI: {uri}"), -1))
        }
        Some(_) => Ok(parsed),
    }
}

pub fn get_millis() -> i64 {
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    ORIGIN.get_or_init(std::time::Instant::now).elapsed().as_millis() as i64
}

pub fn java_min_f(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() { a } else { b }
    } else if a <= b {
        a
    } else {
        b
    }
}

pub fn java_max_f(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() { b } else { a }
    } else if a >= b {
        a
    } else {
        b
    }
}

pub fn java_format_fixed(value: f64, decimals: i32) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let negative = value.is_sign_negative();
    let shortest = format!("{:e}", value.abs());
    let (mantissa, exponent) = shortest.split_once('e').expect("exponent form");
    let mut digits: Vec<u8> = mantissa.bytes().filter(|b| b.is_ascii_digit()).map(|b| b - b'0').collect();
    let mut point = exponent.parse::<i32>().unwrap() + 1;
    let keep = point + decimals;
    if keep < 0 {
        digits = vec![0];
        point = 1;
    } else if (keep as usize) < digits.len() {
        let round_up = digits[keep as usize] >= 5;
        digits.truncate(keep as usize);
        if round_up {
            let mut i = digits.len();
            loop {
                if i == 0 {
                    digits.insert(0, 1);
                    point += 1;
                    break;
                }
                i -= 1;
                if digits[i] == 9 {
                    digits[i] = 0;
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
    }
    let digit = |i: i32| if i >= 0 && (i as usize) < digits.len() { (b'0' + digits[i as usize]) as char } else { '0' };
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if point <= 0 {
        out.push('0');
    } else {
        (0..point).for_each(|i| out.push(digit(i)));
    }
    if decimals > 0 {
        out.push('.');
        (point..point + decimals).for_each(|i| out.push(digit(i)));
    }
    out
}

pub fn shuffle<T>(list: &mut [T], random: &mut impl RandomSource) {
    for i in (2..=list.len()).rev() {
        let swap_to = random.next_int_bounded(i as i32) as usize;
        list.swap(i - 1, swap_to);
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::random::{LegacyRandomSource, RandomSource};

    #[test]
    fn shuffle_matches_vanilla() {
        let mut random = LegacyRandomSource::new(42);
        let mut list: Vec<i32> = (0..10).collect();
        super::shuffle(&mut list, &mut random);
        assert_eq!(list, [4, 6, 2, 1, 7, 9, 8, 5, 3, 0]);
        assert_eq!(random.next_int(), 392236186);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prediction {
    Predicted,
    ServerOnly,
}
