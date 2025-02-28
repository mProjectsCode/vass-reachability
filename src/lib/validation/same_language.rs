use itertools::{Itertools, repeat_n};

use crate::automaton::{Automaton, AutomatonEdge};

/// Checks if two automata accept the same language.
/// This is done by checking if the alphabets are the same and then checking if
/// the automata accept the same words up to a certain length.
pub fn same_language<E: AutomatonEdge>(
    a: &impl Automaton<E>,
    b: &impl Automaton<E>,
    max_word_length: usize,
) -> bool {
    // first we need to check if the alphabets are the same
    if a.alphabet() != b.alphabet() {
        return false;
    }

    for i in 0..max_word_length {
        let combinations = repeat_n(a.alphabet(), i).multi_cartesian_product();

        for word in combinations {
            let word: Vec<E> = word.into_iter().cloned().collect_vec();
            if a.accepts(&word) != b.accepts(&word) {
                println!("{:?}", word);
                return false;
            }
        }
    }

    true
}

pub fn assert_same_language<E: AutomatonEdge>(
    a: &impl Automaton<E>,
    b: &impl Automaton<E>,
    max_word_length: usize,
) {
    // first we need to check if the alphabets are the same
    if a.alphabet() != b.alphabet() {
        panic!("Alphabets are not the same");
    }

    for i in 0..max_word_length {
        let combinations = repeat_n(a.alphabet(), i).multi_cartesian_product();

        for word in combinations {
            let word: Vec<E> = word.into_iter().cloned().collect_vec();
            match (a.accepts(&word), b.accepts(&word)) {
                (true, false) => {
                    panic!(
                        "{:?} is accepted by automaton `a` but not by automaton `b`. Thus their languages are not equal.",
                        word
                    );
                }
                (false, true) => {
                    panic!(
                        "{:?} is accepted by automaton `b` but not by automaton `a`. Thus their languages are not equal.",
                        word
                    );
                }
                _ => {}
            }
        }
    }
}

/// Assert that the language accepted by automaton `a` is the inverse of the
/// language accepted by automaton `b`. Meaning no word is accepted by both and
/// no word is accepted by none.
pub fn assert_inverse_language<E: AutomatonEdge>(
    a: &impl Automaton<E>,
    b: &impl Automaton<E>,
    max_word_length: usize,
) {
    // first we need to check if the alphabets are the same
    if a.alphabet() != b.alphabet() {
        panic!("Alphabets are not the same");
    }

    for i in 0..max_word_length {
        let combinations = repeat_n(a.alphabet(), i).multi_cartesian_product();

        for word in combinations {
            let word: Vec<E> = word.into_iter().cloned().collect_vec();
            match (a.accepts(&word), b.accepts(&word)) {
                (true, true) => {
                    panic!(
                        "{:?} is accepted by automaton `a` and by automaton `b`. Thus their languages are not inverse.",
                        word
                    );
                }
                (false, false) => {
                    panic!(
                        "{:?} is accepted by automaton `b` and by automaton `a`. Thus their languages are not inverse.",
                        word
                    );
                }
                _ => {}
            }
        }
    }
}

/// Assert that the language accepted by automaton `a` is a subset of the
/// language accepted by automaton `b`.
pub fn assert_subset_language<E: AutomatonEdge>(
    a: &impl Automaton<E>,
    b: &impl Automaton<E>,
    max_word_length: usize,
) {
    // first we need to check if the alphabets are the same
    if a.alphabet() != b.alphabet() {
        panic!("Alphabets are not the same");
    }

    for i in 0..max_word_length {
        let combinations = repeat_n(a.alphabet(), i).multi_cartesian_product();

        for word in combinations {
            let word: Vec<E> = word.into_iter().cloned().collect_vec();

            #[allow(clippy::single_match)]
            match (a.accepts(&word), b.accepts(&word)) {
                (true, false) => {
                    panic!(
                        "{:?} is accepted by automaton `a` but not by automaton `b`. Thus the language of `a` is not a subset of `b`.",
                        word
                    );
                }
                _ => {}
            }
        }
    }
}
