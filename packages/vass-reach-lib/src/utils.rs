use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use serde::Serialize;

/// Replaces the element at `index` in `vec` with the elements produced by `f`.
pub fn replace_with_many<T, I, F>(vec: &mut Vec<T>, index: usize, f: F)
where
    F: FnOnce(T) -> I,
    I: IntoIterator<Item = T>,
{
    let after = vec.split_off(index + 1);
    let to_insert = f(vec.pop().expect("Index out of bounds"));
    vec.extend(to_insert);
    vec.extend(after);
}

pub fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub fn sanitize_path_component(input: &str, fallback: &str) -> String {
    let sanitized: String = input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        fallback.to_string()
    } else {
        sanitized
    }
}

pub fn write_json_pretty_atomic<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let tmp_path = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&tmp_path, bytes)
        .with_context(|| format!("failed writing trace file: {}", tmp_path.display()))?;
    fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "failed to finalize trace file rename from {} to {}",
            tmp_path.display(),
            path.display()
        )
    })?;
    Ok(())
}

#[test]
fn test_replace_with_many() {
    let mut vec = vec![1, 2, 3, 4];
    replace_with_many(&mut vec, 1, |x| vec![x * 5, x * 10]);
    assert_eq!(vec, vec![1, 10, 20, 3, 4]);
}

#[test]
fn test_sanitize_path_component() {
    assert_eq!(sanitize_path_component("foo/bar", "fallback"), "foo_bar");
    assert_eq!(sanitize_path_component("", "fallback"), "fallback");
    assert_eq!(
        sanitize_path_component("run.name-1", "fallback"),
        "run.name-1"
    );
}
