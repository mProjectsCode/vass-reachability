use std::time::Duration;

use vass_reachability::{
    automaton::petri_net::InitializedPetriNet, solver::vass_reach::VASSReachSolverOptions,
};

// Seems like there are currently two types of issues:
// 1. longer and longer negative paths are found, see net 25. We probably need to cut away some part of the loop.
// 2. Paths repeatedly overflowing mu. Seems like increasing mu to get rid of simple paths is not always the solution.

fn main() {
    // let mut petri_net = PetriNet::new(3);

    // petri_net.add_transition(vec![], vec![(2, 1)]);
    // petri_net.add_transition(vec![(1, 1), (1, 2)], vec![(2, 2), (2, 3)]);
    // petri_net.add_transition(vec![(2, 3)], vec![(2, 1), (1, 2)]);

    // let initialized_vass = petri_net.init(vec![1, 0, 2], vec![1, 2, 2]).to_vass();

    let initialized_vass =
        InitializedPetriNet::from_file("test_data/petri_nets/3/unknown_2.json").to_vass();

    let res = VASSReachSolverOptions::default()
        .with_iteration_limit(100)
        .with_time_limit(Duration::from_secs(10))
        .with_log_level(vass_reachability::logger::LogLevel::Debug)
        .with_log_file("logs/log.txt")
        .to_solver(initialized_vass)
        .solve_n();

    dbg!(&res);
    // assert!(!res.unknown());
}
