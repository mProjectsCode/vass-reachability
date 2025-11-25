use std::time::Duration;

use vass_reach_lib::{
    automaton::petri_net::{
        PetriNet,
        initialized::InitializedPetriNet,
        spec::{PetriNetSpec, ToSpecFormat},
    },
    config::VASSReachConfig,
    solver::vass_reach::VASSReachSolver,
};

#[test]
fn parse_from_spec_1() {
    let spec_str = r#"
    vars
        p1 p2 p3
    rules
        p1 >= 1 ->
            p1' = p1-1,
            p2' = p2+1;
        p2 >= 1 ->
            p2' = p2-1,
            p3' = p3+1;
    init
        p1=2, p2=0, p3=0
    target
        p1=0, p2=0, p3=2"#;

    let spec = PetriNetSpec::parse(spec_str).unwrap();
    let net = InitializedPetriNet::try_from(spec).unwrap();

    let res = VASSReachSolver::new(
        &net.to_vass(),
        // some time that is long enough, but makes the test run in a reasonable time
        VASSReachConfig::default().with_timeout(Some(Duration::from_secs(5))),
        None,
    )
    .solve();

    assert!(res.is_success());
}

#[test]
fn parse_from_spec_2() {
    let spec_str = r#"
    vars
        p1 p2 p3
    rules
        p1 >= 1 ->
            p3' = p1-1,
            p2' = p2+1;
        p2 >= 1 ->
            p2' = p2-1,
            p3' = p3+1;
    init
        p1=2, p2=0, p3=0
    target
        p1=0, p2=0, p3=2"#;

    let spec = PetriNetSpec::parse(spec_str).unwrap();
    // an update copying tokens from p1 to p3 is not allowed in a Petri net
    assert!(InitializedPetriNet::try_from(spec).is_err());
}

#[test]
fn parse_from_spec_3() {
    let spec_str = r#"
    vars
        p1 p2 p3
    rules
        p1 >= 1 ->
            p1' = p4-1,
            p2' = p2+1;
        p2 >= 1 ->
            p2' = p2-1,
            p3' = p3+1;
    init
        p1=2, p2=0, p3=0
    target
        p1=0, p2=0, p3=2"#;

    let spec = PetriNetSpec::parse(spec_str).unwrap();
    // an update referring to a non-existing variable p4
    assert!(InitializedPetriNet::try_from(spec).is_err());
}

#[test]
fn parse_from_spec_4() {
    let spec_str = r#"
    vars
        p1 p2 p3
    rules
        p1 >= 1 ->
            p1' = p1-1,
            p2' = p2+1;
        p2 >= 1 ->
            p2' = p2-1,
            p3' = p3+1;
    init
        p1=2, p2=0, p4=0
    target
        p1=0, p2=0, p3=2"#;

    let spec = PetriNetSpec::parse(spec_str).unwrap();
    // init referring to a non-existing variable p4
    assert!(InitializedPetriNet::try_from(spec).is_err());
}

#[test]
fn parse_from_spec_5() {
    let spec_str = r#"
    vars
        p1 p2 p3
    rules
        p1 >= 1 ->
            p1' = p1-1,
            p2' = p2+1;
        p2 >= 1 ->
            p2' = p2-1,
            p3' = p3+1;
    init
        p1=2, p2=0, p3=0
    target
        p1=0, p2=0, p4=2"#;

    let spec = PetriNetSpec::parse(spec_str).unwrap();
    // target referring to a non-existing variable p4
    assert!(InitializedPetriNet::try_from(spec).is_err());
}

#[test]
fn parse_and_stringify() {
    let spec_str = r#"
vars
    p1 p2 p3
rules
    p1 >= 1 ->
        p1' = p1-1,
        p2' = p2+1;
    p2 >= 1 ->
        p2' = p2-1,
        p3' = p3+1;
init
    p1=2, p2=0, p3=0
target
    p1=0, p2=0, p3=2"#;

    let spec = PetriNetSpec::parse(spec_str).unwrap();
    let net = InitializedPetriNet::try_from(spec).unwrap();
    let stringified = net.to_spec_format();

    assert_eq!(stringified.trim(), spec_str.trim());
}

#[test]
fn stringify_and_parse() {
    let mut net = PetriNet::new(2);
    net.add_transition(vec![], vec![(1, 1)]);
    net.add_transition(vec![(1, 2)], vec![(2, 2)]);

    let initialized_net = InitializedPetriNet::new(net, vec![0, 1].into(), vec![2, 2].into());

    let spec_str = initialized_net.to_spec_format();
    let parsed_net = InitializedPetriNet::parse_from_spec(&spec_str);

    assert_eq!(parsed_net.unwrap(), initialized_net);
}
