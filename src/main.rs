use vass_reachability::automaton::petri_net::PetriNet;

fn main() {
    let mut petri_net = PetriNet::new(3);

    petri_net.add_transition(vec![], vec![(2, 1)]);
    petri_net.add_transition(vec![(1, 1), (1, 2)], vec![(2, 2), (2, 3)]);
    petri_net.add_transition(vec![(2, 3)], vec![(2, 1), (1, 2)]);

    let initialized_petri_net = petri_net.init(vec![1, 0, 2], vec![1, 2, 2]);

    let initialized_vass = initialized_petri_net.to_vass();

    assert!(!initialized_vass.reach_1());
}
