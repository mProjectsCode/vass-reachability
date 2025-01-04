// use vass_reachability::automaton::{modulo::ModuloDFA, Automaton};

// #[test]
// fn test_modulo() {
//     let modulo = ModuloDFA::new(2, 3, false);

//     // dbg!(&modulo);

//     let input = vec![1, -2, 2, -1];
//     assert!(!modulo.accepts(&input));

//     let input = vec![1, -2, 2];
//     assert!(modulo.accepts(&input));

//     let input = vec![1, 1, 1];
//     assert!(!modulo.accepts(&input));

//     let input = vec![-2, -2, -2];
//     assert!(!modulo.accepts(&input));
// }
