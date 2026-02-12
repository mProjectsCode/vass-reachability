/// In this file, we parse textual `spec` representations of Petri nets.
///
/// The format is described [here](https://github.com/pierreganty/mist/wiki#input-format-of-mist).
/// An example Petri net spec is as follows:
/// /// ```
/// /// vars
/// ///     p1 p2 p3
/// /// rules
/// ///     p1 >= 1 ->
/// ///         p1' = p1-1,
/// ///         p2' = p2+1;
/// ///     p2 >= 1 ->
/// ///         p2' = p2-1,
/// ///         p3' = p3+1;
/// /// init
/// ///     p1=2, p2=0, p3=0
/// /// target
/// ///     p1=0, p2=0, p3=2
/// /// ```
///
/// We only support Petri nets where the updates only modify the counter
/// itself (i.e., no transfer of tokens between places).
///
/// We also don't support invariants on places (only guards on transitions).
///
/// For init and target, we only support equality constraints (only
/// reachability, not coverability). Unnamed places are assumed to have value 0
/// in init and target.
use nom::{Parser, bytes::complete::tag, character::complete::space1, error::ParseError};

use crate::automaton::{
    petri_net::{initialized::InitializedPetriNet, transition::PetriNetTransition},
    vass::counter::VASSCounterValuation,
};

fn integer<'a, E: ParseError<&'a str>>(input: &'a str) -> nom::IResult<&'a str, i32, E> {
    let (input, num_str) = nom::character::complete::digit1(input)?;
    let num = num_str.parse::<i32>().unwrap();
    Ok((input, num))
}

fn opt_whitespace<'a, E: ParseError<&'a str>>(input: &'a str) -> nom::IResult<&'a str, &'a str, E> {
    nom::character::complete::multispace0(input)
}

fn whitespace<'a, E: ParseError<&'a str>>(input: &'a str) -> nom::IResult<&'a str, &'a str, E> {
    nom::character::complete::multispace1(input)
}

fn separator<'a, E: ParseError<&'a str>>(input: &'a str) -> nom::IResult<&'a str, (), E> {
    let (input, _) = opt_whitespace(input)?;
    let (input, _) = tag(",")(input)?;
    let (input, _) = opt_whitespace(input)?;
    Ok((input, ()))
}

fn variable<'a, E: ParseError<&'a str>>(input: &'a str) -> nom::IResult<&'a str, &'a str, E> {
    let (input2, (first, rest)) = (
        nom::character::complete::alpha1,
        nom::character::complete::alphanumeric0,
    )
        .parse(input)?;

    Ok((input2, &input[..first.len() + rest.len()]))
}

// E.g., x1 x2 x3
fn set_of_vars<'a, E: ParseError<&'a str>>(
    input: &'a str,
) -> nom::IResult<&'a str, Vec<&'a str>, E> {
    nom::multi::separated_list1(space1, variable).parse(input)
}

#[derive(Debug, Clone)]
pub struct GuardAtom<'a> {
    pub var: &'a str,
    pub value: i32,
}

fn guard_atom<'a, E: ParseError<&'a str>>(
    input: &'a str,
) -> nom::IResult<&'a str, GuardAtom<'a>, E> {
    let (input, var) = variable(input)?;
    let (input, _) = opt_whitespace(input)?;
    let (input, _) = tag(">=")(input)?;
    let (input, _) = opt_whitespace(input)?;
    let (input, value) = integer(input)?;

    Ok((input, GuardAtom { var, value }))
}

#[test]
fn test_guard_atom_1() {
    let input = "p1 >= 3";
    let (_, atom) = guard_atom::<nom::error::Error<&str>>(input).unwrap();
    assert_eq!(atom.var, "p1");
    assert_eq!(atom.value, 3);
}

#[test]
fn test_guard_atom_2() {
    let input = "abc12 >= 34";
    let (_, atom) = guard_atom::<nom::error::Error<&str>>(input).unwrap();
    assert_eq!(atom.var, "abc12");
    assert_eq!(atom.value, 34);
}

#[derive(Debug, Clone)]
pub struct Guard<'a> {
    pub atoms: Vec<GuardAtom<'a>>,
}

impl<'a> Guard<'a> {
    pub fn to_counter_valuation(
        &self,
        variables: &[&'a str],
    ) -> anyhow::Result<VASSCounterValuation> {
        let mut valuation = vec![0; variables.len()];

        for atom in &self.atoms {
            if let Some(pos) = variables.iter().position(|&v| v == atom.var) {
                valuation[pos] = atom.value;
            } else {
                return Err(anyhow::anyhow!(
                    "Variable '{}' in guard not found in variable list.",
                    atom.var
                ));
            }
        }

        Ok(valuation.into())
    }
}

fn guard<'a, E: ParseError<&'a str>>(input: &'a str) -> nom::IResult<&'a str, Guard<'a>, E> {
    let (input, atoms) = nom::multi::separated_list1(separator, guard_atom).parse(input)?;

    Ok((input, Guard { atoms }))
}

#[test]
fn test_guard_1() {
    let input = "p1 >= 3, p2 >= 0, p3 >= 5";
    let (_, guard) = guard::<nom::error::Error<&str>>(input).unwrap();
    assert_eq!(guard.atoms.len(), 3);
    assert_eq!(guard.atoms[0].var, "p1");
    assert_eq!(guard.atoms[0].value, 3);
    assert_eq!(guard.atoms[1].var, "p2");
    assert_eq!(guard.atoms[1].value, 0);
    assert_eq!(guard.atoms[2].var, "p3");
    assert_eq!(guard.atoms[2].value, 5);
}

#[derive(Debug, Clone)]
pub struct Update<'a> {
    pub target: &'a str,
    pub source: &'a str,
    pub change: i32,
}

fn update<'a, E: ParseError<&'a str>>(input: &'a str) -> nom::IResult<&'a str, Update<'a>, E> {
    let (input, target) = variable(input)?;
    let (input, _) = tag("'")(input)?;
    let (input, _) = opt_whitespace(input)?;
    let (input, _) = tag("=")(input)?;
    let (input, _) = opt_whitespace(input)?;
    let (input, source) = variable(input)?;
    let (input, _) = opt_whitespace(input)?;
    let (input, sign) = nom::branch::alt((tag("+"), tag("-"))).parse(input)?;
    let (input, _) = opt_whitespace(input)?;
    let (input, value) = integer(input)?;

    let change = if sign == "+" { value } else { -value };

    Ok((
        input,
        Update {
            target,
            source,
            change,
        },
    ))
}

#[test]
fn test_update_1() {
    let input = "p2' = p2 + 1";
    let (_, update) = update::<nom::error::Error<&str>>(input).unwrap();
    assert_eq!(update.target, "p2");
    assert_eq!(update.source, "p2");
    assert_eq!(update.change, 1);
}

#[test]
fn test_update_2() {
    let input = "p3' = a1-5";
    let (_, update) = update::<nom::error::Error<&str>>(input).unwrap();
    assert_eq!(update.target, "p3");
    assert_eq!(update.source, "a1");
    assert_eq!(update.change, -5);
}

#[derive(Debug, Clone)]
pub struct TransitionSpec<'a> {
    pub guard: Guard<'a>,
    pub updates: Vec<Update<'a>>,
}

impl<'a> TransitionSpec<'a> {
    pub fn to_transition(&self, variables: &[&'a str]) -> anyhow::Result<PetriNetTransition> {
        let mut input = vec![0; variables.len()];
        let mut output = vec![0; variables.len()];

        for atom in &self.guard.atoms {
            if atom.value < 0 {
                anyhow::bail!(
                    "Guard atom for variable '{}' has negative value {}. Only non-negative values are supported.",
                    atom.var,
                    atom.value
                );
            }

            if let Some(pos) = variables.iter().position(|&v| v == atom.var) {
                input[pos] = -atom.value;
            } else {
                anyhow::bail!(
                    "Variable '{}' in guard not found in variable list.",
                    atom.var
                );
            }
        }

        for update in &self.updates {
            if update.source != update.target {
                anyhow::bail!(
                    "Unsupported update from '{}' to '{}'. Only changes to the counter itself are supported.",
                    update.source,
                    update.target
                );
            }

            let pos = variables.iter().position(|&v| v == update.source);
            if let Some(pos) = pos {
                let guard_value = input[pos];
                if update.change < 0 {
                    // Consuming tokens
                    if update.change < guard_value {
                        anyhow::bail!(
                            "Cannot consume {} tokens from variable '{}' which has only {} tokens in the guard.",
                            -update.change,
                            update.source,
                            -guard_value
                        );
                    }
                }
                output[pos] = -guard_value + update.change;
            } else {
                anyhow::bail!(
                    "Variable '{}' in update not found in variable list.",
                    update.source
                );
            }
        }

        Ok(PetriNetTransition::from_vass_updates(&input, &output))
    }
}

fn transition<'a, E: ParseError<&'a str>>(
    input: &'a str,
) -> nom::IResult<&'a str, TransitionSpec<'a>, E> {
    let (input, guard) = guard(input)?;
    let (input, _) = opt_whitespace(input)?;
    let (input, _) = tag("->")(input)?;
    let (input, _) = opt_whitespace(input)?;
    let (input, updates) = nom::multi::separated_list1(separator, update).parse(input)?;
    let (input, _) = tag(";")(input)?;

    Ok((input, TransitionSpec { guard, updates }))
}

#[test]
fn test_transition_1() {
    let input = r#"p1 >= 1, p2 >= 0 ->
        p1' = p1 - 1,
        p2' = p2 + 1;"#;

    let (_, transition) = transition::<nom::error::Error<&str>>(input).unwrap();
    assert_eq!(transition.guard.atoms.len(), 2);
    assert_eq!(transition.updates.len(), 2);
}

fn eq_guard_atom<'a, E: ParseError<&'a str>>(
    input: &'a str,
) -> nom::IResult<&'a str, GuardAtom<'a>, E> {
    let (input, var) = variable(input)?;
    let (input, _) = opt_whitespace(input)?;
    let (input, _) = tag("=")(input)?;
    let (input, _) = opt_whitespace(input)?;
    let (input, value) = integer(input)?;

    Ok((input, GuardAtom { var, value }))
}

fn eq_guard<'a, E: ParseError<&'a str>>(input: &'a str) -> nom::IResult<&'a str, Guard<'a>, E> {
    let (input, atoms) = nom::multi::separated_list1(separator, eq_guard_atom).parse(input)?;

    Ok((input, Guard { atoms }))
}

fn vars<'a, E: ParseError<&'a str>>(input: &'a str) -> nom::IResult<&'a str, Vec<&'a str>, E> {
    let (input, _) = opt_whitespace(input)?;
    let (input, _) = tag("vars")(input)?;
    let (input, _) = whitespace(input)?;

    set_of_vars(input)
}

#[test]
fn test_vars_1() {
    let input = r#"
    vars
        p1 p2 p3
    "#;

    let (_, vars) = vars::<nom::error::Error<&str>>(input).unwrap();
    assert_eq!(vars, vec!["p1", "p2", "p3"]);
}

fn rules<'a, E: ParseError<&'a str>>(
    input: &'a str,
) -> nom::IResult<&'a str, Vec<TransitionSpec<'a>>, E> {
    let (input, _) = opt_whitespace(input)?;
    let (input, _) = tag("rules")(input)?;
    let (input, _) = whitespace(input)?;

    nom::multi::separated_list1(opt_whitespace, transition).parse(input)
}

#[test]
fn test_rules_1() {
    let input = r#"
    rules
        p1 >= 1 ->
            p1' = p1-1,
            p2' = p2+1;
    "#;

    let (_, rules) = rules::<nom::error::Error<&str>>(input).unwrap();
    assert_eq!(rules.len(), 1);
}

#[test]
fn test_rules_2() {
    let input = r#"
    rules
        p1 >= 1 ->
            p1' = p1-1,
            p2' = p2+1;
        p2 >= 1 ->
            p2' = p2-1,
            p3' = p3+1;
    "#;

    let (_, rules) = rules::<nom::error::Error<&str>>(input).unwrap();
    assert_eq!(rules.len(), 2);
}

fn init<'a, E: ParseError<&'a str>>(input: &'a str) -> nom::IResult<&'a str, Guard<'a>, E> {
    let (input, _) = opt_whitespace(input)?;
    let (input, _) = tag("init")(input)?;
    let (input, _) = whitespace(input)?;

    eq_guard(input)
}

#[test]
fn test_init_1() {
    let input = r#"
    init
        p1=2, p2=0, p3=0
    "#;
    let (_, init_guard) = init::<nom::error::Error<&str>>(input).unwrap();
    assert_eq!(init_guard.atoms.len(), 3);
    assert_eq!(init_guard.atoms[0].var, "p1");
    assert_eq!(init_guard.atoms[0].value, 2);
    assert_eq!(init_guard.atoms[1].var, "p2");
    assert_eq!(init_guard.atoms[1].value, 0);
    assert_eq!(init_guard.atoms[2].var, "p3");
    assert_eq!(init_guard.atoms[2].value, 0);
}

fn target<'a, E: ParseError<&'a str>>(input: &'a str) -> nom::IResult<&'a str, Guard<'a>, E> {
    let (input, _) = opt_whitespace(input)?;
    let (input, _) = tag("target")(input)?;
    let (input, _) = whitespace(input)?;

    eq_guard(input)
}

#[test]
fn test_target_1() {
    let input = r#"
    target
        p1=2, p2=0, p3=0
    "#;
    let (_, target_guard) = target::<nom::error::Error<&str>>(input).unwrap();
    assert_eq!(target_guard.atoms.len(), 3);
    assert_eq!(target_guard.atoms[0].var, "p1");
    assert_eq!(target_guard.atoms[0].value, 2);
    assert_eq!(target_guard.atoms[1].var, "p2");
    assert_eq!(target_guard.atoms[1].value, 0);
    assert_eq!(target_guard.atoms[2].var, "p3");
    assert_eq!(target_guard.atoms[2].value, 0);
}

#[derive(Debug, Clone)]
pub struct PetriNetSpec<'a> {
    pub variables: Vec<&'a str>,
    pub rules: Vec<TransitionSpec<'a>>,
    pub initial: Guard<'a>,
    pub target: Guard<'a>,
}

impl<'a> PetriNetSpec<'a> {
    fn p(input: &'a str) -> nom::IResult<&'a str, PetriNetSpec<'a>, nom::error::Error<&'a str>> {
        let (input, variables) = vars(input)?;
        let (input, rules) = rules(input)?;
        let (input, initial) = init(input)?;
        let (input, target) = target(input)?;
        let (input, _) = opt_whitespace(input)?;

        Ok((
            input,
            PetriNetSpec {
                variables,
                rules,
                initial,
                target,
            },
        ))
    }

    pub fn parse(input: &'a str) -> anyhow::Result<PetriNetSpec<'a>> {
        match Self::p(input) {
            Ok(spec) => Ok(spec.1),
            Err(e) => Err(anyhow::anyhow!("Failed to parse Petri net spec: {}", e)),
        }
    }
}

#[test]
fn test_spec_1() {
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
    let (_, spec) = PetriNetSpec::p(&spec_str).unwrap();
    assert_eq!(spec.variables, vec!["p1", "p2", "p3"]);
    assert_eq!(spec.rules.len(), 2);
    assert_eq!(spec.initial.atoms.len(), 3);
    assert_eq!(spec.target.atoms.len(), 3);
}

pub trait ToSpecFormat {
    fn to_spec_format(&self) -> String;
}

impl ToSpecFormat for InitializedPetriNet {
    fn to_spec_format(&self) -> String {
        let mut spec = String::new();

        // vars
        spec.push_str("vars\n    ");
        let vars = (1..=self.net.place_count)
            .map(|i| format!("p{}", i))
            .collect::<Vec<String>>()
            .join(" ");
        spec.push_str(&vars);
        spec.push('\n');

        // rules
        spec.push_str("rules\n");
        for transition in &self.net.transitions {
            spec.push_str("    ");

            // guard
            let mut guard_atoms = vec![];
            for (weight, place) in &transition.input {
                guard_atoms.push(format!("p{} >= {}", place, weight));
            }
            if guard_atoms.is_empty() {
                guard_atoms.push("p1 >= 0".to_string());
            }
            spec.push_str(&guard_atoms.join(", "));
            spec.push_str(" ->\n        ");

            // updates
            let mut updates = vec![];
            for i in 1..=self.net.place_count {
                let (input, output) = transition.get_update_for_place(i);

                let change = output as i32 - input as i32;
                if change != 0 {
                    let sign = if change > 0 { "+" } else { "-" };
                    updates.push(format!("p{}' = p{}{}{}", i, i, sign, change.abs()));
                }
            }
            if updates.is_empty() {
                updates.push("p1' = p1+0".to_string());
            }
            spec.push_str(&updates.join(",\n        "));
            spec.push_str(";\n");
        }

        // init
        spec.push_str("init\n    ");
        let mut init_atoms = vec![];
        let init_valuation = &self.initial_marking;
        for i in 0..self.net.place_count {
            init_atoms.push(format!("p{}={}", i + 1, init_valuation[i]));
        }
        spec.push_str(&init_atoms.join(", "));
        spec.push('\n');

        // target
        spec.push_str("target\n    ");
        let mut target_atoms = vec![];
        let target_valuation = &self.final_marking;
        for i in 0..self.net.place_count {
            target_atoms.push(format!("p{}={}", i + 1, target_valuation[i]));
        }
        spec.push_str(&target_atoms.join(", "));
        spec.push('\n');

        spec
    }
}
