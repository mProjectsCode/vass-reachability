# TODO

- [x] Idea: chance based on how often a SCC was visited, when long in SCC, then maybe more safe?
  Done: Due to the way we choose SCCs to add, we do this implicitly.

- [x] Build the initial MGTS from the SCCs of visited subgraph.

- [ ] Extend the (partial bc we looked only at the visited subgraph) SCCs in the MGTS by adding more edges that weren't visited

- [ ] Idea: Sub SCC, we look at strongly connected subsets of SCCs.
  Probably the easiest would be to look at the path through the SCC (or just the parikh image) if it is reachable, then do a sub-refinement step where we don't disregard the SCC, but instead look if we can remove some edge or node from the SCC to make it unreachable again.

- [ ] Idea: Why are we including the modulo automatons in the MGTS?
  1. they do influence SCCs, but we can probably work around that, capturing the relevant nodes with the modulo automatons, but then when translating to a MGTS, we can disregard them and get smaller automatons
  2. when solving the MGTS, the modulo automatons are strictly weaker than the Z-Reachability we search for, so we can disregard them as well.
  3. When building the automaton in the end we want to construct it in a way that we restrict as much as possible. But the current way we construct them (just take the MGTS and invert it) means that by restricting the MGTS more, we make the rejected language by the final automaton smaller (and the automaton bigger). This is not great, as yeah, we are more precise, but we already have the precision in the MGTS. We are just adding more states to the automaton, which makes it harder to handle.
  - turns out we need some way of writing modulo values to specific SCCs in the MGTS. But I think we can encode them better than using automatons. Z3 should allow us to encode these modulo constraints directly.

- [ ] ignore some maybe not all LGS. (maybe sleep some MGTSs)

- [ ] find difficult to solve instances
