use std::time::Duration;

use vass_reachability::{
    automaton::petri_net::PetriNet, boxed_slice, solver::vass_reach::VASSReachSolverOptions,
};
// Seems like there are currently two types of issues:
// 1. longer and longer negative paths are found, see net 25. We probably need
//    to cut away some part of the loop.
// 2. Paths repeatedly overflowing mu. Seems like increasing mu to get rid of
//    simple paths is not always the solution.

fn main() {
    // let initialized_vass =
    //     InitializedPetriNet::from_file("test_data/petri_nets/3/unknown_2.json").
    // to_vass();

    let mut petri_net = PetriNet::new(4);

    petri_net.add_transition(vec![(1, 1)], vec![(1, 2)]);
    petri_net.add_transition(vec![(1, 2)], vec![(1, 3), (1, 4)]);
    petri_net.add_transition(vec![(1, 3)], vec![(1, 2)]);

    let initialized_petri_net = petri_net.init(boxed_slice![1, 0, 0, 0], boxed_slice![0, 1, 0, 3]);

    let initialized_vass = initialized_petri_net.to_vass();

    let res = VASSReachSolverOptions::default()
        .with_iteration_limit(100)
        .with_time_limit(Duration::from_secs(1))
        .with_log_level(vass_reachability::logger::LogLevel::Debug)
        .with_log_file("logs/log.txt")
        .to_solver(initialized_vass)
        .solve();

    // let mut cfg = initialized_vass.to_cfg();
    // cfg.add_failure_state(vec![]);
    // cfg = cfg.minimize();
    // let res = solve_z_reach_for_cfg(&cfg, &initialized_vass.initial_valuation,
    // &initialized_vass.final_valuation, None);

    dbg!(&res);
    // assert!(!res.unknown());
}
