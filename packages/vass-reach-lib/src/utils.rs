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

#[test]
fn test_replace_with_many() {
    let mut vec = vec![1, 2, 3, 4];
    replace_with_many(&mut vec, 1, |x| vec![x * 5, x * 10]);
    assert_eq!(vec, vec![1, 10, 20, 3, 4]);
}
