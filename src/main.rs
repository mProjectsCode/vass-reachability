use vass_reachability::{
    automaton::petri_net::PetriNet, solver::vass_reach::VASSReachSolverOptions,
};

fn main() {
    let mut petri_net = PetriNet::new(3);

    petri_net.add_transition(vec![], vec![(2, 1)]);
    petri_net.add_transition(vec![(1, 1), (1, 2)], vec![(2, 2), (2, 3)]);
    petri_net.add_transition(vec![(2, 3)], vec![(2, 1), (1, 2)]);

    let initialized_vass = petri_net.init(vec![1, 0, 2], vec![1, 2, 2]).to_vass();

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
        .to_solver(initialized_vass)
        .solve_n();

    assert!(!res.reachable());
}
