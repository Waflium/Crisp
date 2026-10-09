pub fn is_valid_path(path: &str) -> bool {
    path.bytes().all(|c| matches!(c, b'_' | b'-' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'.'))
}

pub fn is_valid_namespace(namespace: &str) -> bool {
    namespace != ".." && namespace.bytes().all(|c| matches!(c, b'_' | b'-' | b'a'..=b'z' | b'0'..=b'9' | b'.'))
}

fn assert_valid_namespace(namespace: &str, path: &str) -> Result<(), String> {
    if is_valid_namespace(namespace) { Ok(()) } else { Err(format!("Non [a-z0-9_.-] character in namespace of identifier: {namespace}:{path}")) }
}

fn assert_valid_path(namespace: &str, path: &str) -> Result<(), String> {
    if is_valid_path(path) { Ok(()) } else { Err(format!("Non [a-z0-9/._-] character in path of location: {namespace}:{path}")) }
}

pub fn by_separator(identifier: &str, separator: char) -> Result<String, String> {
    let (namespace, path) = match identifier.find(separator) {
        Some(0) => ("minecraft", &identifier[1..]),
        Some(i) => {
            let (namespace, path) = (&identifier[..i], &identifier[i + 1..]);
            assert_valid_namespace(namespace, path)?;
            (namespace, path)
        }
        None => ("minecraft", identifier),
    };
    assert_valid_path(namespace, path)?;
    Ok(format!("{namespace}:{path}"))
}

pub fn parse(identifier: &str) -> Result<String, String> {
    by_separator(identifier, ':')
}

pub fn read(input: &str) -> Result<String, String> {
    parse(input).map_err(|e| format!("Not a valid resource location: {input} {e}"))
}
