use crate::automaton::{AutEdge, Automaton};
use itertools::{repeat_n, Itertools};

/// Checks if two automata accept the same language.
/// This is done by checking if the alphabets are the same and then checking if the automata accept the same words up to a certain length.
pub fn same_language<E: AutEdge>(
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

pub fn assert_same_language<E: AutEdge>(
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
                    panic!("{:?} is accepted by a but not by b", word);
                }
                (false, true) => {
                    panic!("{:?} is accepted by b but not by a", word);
                }
                _ => {}
            }

            if a.accepts(&word) != b.accepts(&word) {
                println!("{:?}", word);
            }
        }
    }
}
