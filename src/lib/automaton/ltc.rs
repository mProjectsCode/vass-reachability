/// LTC (Loop Transition Chain) is more or less a GTS (Graph Transition System) with only a single loop for the graphs.
/// This implementation is specifically for VASS.
pub struct LTC {
    pub elements: Vec<LTCElement>,
}

impl LTC {
    pub fn new() -> Self {
        LTC { elements: vec![] }
    }

    pub fn add_loop(&mut self, loop_: (Vec<i32>, Vec<i32>)) {
        match self.elements.last() {
            Some(LTCElement::Loop(_)) => panic!("Cannot have two loops in a row"),
            _ => (),
        }

        self.elements.push(LTCElement::Loop(loop_));
    }

    pub fn add_transition(&mut self, transition: (Vec<i32>, Vec<i32>)) {
        self.elements.push(LTCElement::Transition(transition));
    }

    /// Reachability from 0 to 0 in the whole numbers, so intermediate valuations may be negative.
    pub fn reach_z(&self) -> bool {
        unimplemented!()
    }

    /// Reachability from 0 to 0 in the natural numbers, so no intermediate valuation may be negative.
    pub fn reach_n(&self) -> bool {
        // this can probably be done with a solver. We need to look at intermediate valuations after each element.
        // if we constrain these to be positive, we might have an algorithm for this.

        unimplemented!()
    }
}

/// A single element in the LTC.
/// Either a loop or a transition.
/// A loop can be taken a any number of times including zero.
/// A transition must be taken exactly once.
/// The first vector needs to be subtracted from the counters and the second vector needs to be added to the counters.
/// Similar to a firing rule in a Petri net.
pub enum LTCElement {
    Loop((Vec<i32>, Vec<i32>)),
    Transition((Vec<i32>, Vec<i32>)),
}
