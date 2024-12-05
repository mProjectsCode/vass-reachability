use vass_reachability::automaton::petri_net::PetriNet;

fn main() {
    let mut petri_net = PetriNet::new(4);

    petri_net.add_transition(vec![(1, 1)], vec![(1, 2)]);
    petri_net.add_transition(vec![(1, 3)], vec![(1, 2)]);
    petri_net.add_transition(vec![(1, 2)], vec![(1, 3), (1, 4)]);

    let initialized_petri_net = petri_net.init(vec![1, 0, 0, 0], vec![0, 1, 0, 3]);

    let initialized_vass = initialized_petri_net.to_vass();

    // dbg!(&initialized_vass);

    assert!(initialized_vass.reach_1());
}
