/// Creates a boxed slice from a list of elements.
#[macro_export]
macro_rules! boxed_slice {
    ($($x:expr),*) => (
        vec![$($x),*].into_boxed_slice()
    );
}
