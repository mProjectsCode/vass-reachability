// how do we represent the counter updates as labels?
// we can represent the counter updates as a tuple of two integers, the first integer is the counter to update, the second integer is the value to update the counter by
// or maybe we can stick with vectors?
// no, because that would inflate our alphabet
// we can represent them as integers from starting from 1 to represent incrementing the counter i and -i to represent decrementing the counter i

use super::Automaton;

/// # Dyck Vass
///
/// The alphabet of the Dyck VASS are symbols that increment and decrement a specific counter.
///
/// An accepting run in the Dyck VASS is a run that starts and ends with all counters at 0 and never goes below 0 in any counter.
///
/// Here increasing a counter `i ∈ [1; size]` is done by the `i32` value `i` and decreasing a counter `i` is done by `-i`.
/// So `5_i32` increments counter 5 and `-5_i32` decrements counter 5.
#[derive(Debug, Clone)]
pub struct DyckVASS {
    alphabet: Vec<i32>,
    size: usize,
}

impl DyckVASS {
    pub fn new(size: usize) -> Self {
        let mut alphabet = vec![];
        for i in 1..=size {
            alphabet.push(i as i32);
            alphabet.push(-(i as i32));
        }

        Self { alphabet, size }
    }
}

impl Automaton<i32> for DyckVASS {
    fn accepts(&self, input: &[i32]) -> bool {
        let mut state: Vec<i32> = vec![0; self.size];

        for symbol in input {
            if !self.alphabet.contains(symbol) {
                panic!("Symbol not in alphabet");
            }

            if *symbol > 0 {
                state[(*symbol - 1) as usize] += 1;
            } else {
                state[(-*symbol - 1) as usize] -= 1;
                if state[(-*symbol - 1) as usize] < 0 {
                    return false;
                }
            }
        }

        state.iter().all(|&x| x == 0)
    }

    fn alphabet(&self) -> &Vec<i32> {
        &self.alphabet
    }
}
