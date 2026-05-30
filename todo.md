# TODO

- [x] Idea: chance based on how often a SCC was visited, when long in SCC, then maybe more safe?
  Done: Due to the way we choose SCCs to add, we do this implicitly.

- [x] Build the initial LinearGraph from the SCCs of visited subgraph.

- [x] Extend the (partial bc we looked only at the visited subgraph) SCCs in the LinearGraph by adding more edges that weren't visited

- [ ] Build a data structure represents the product of a subset of automatons in an implicit product. We would use this to more easily ignore the counting cfg and to ignore/sleep certain LinearGraph

- [ ] Better debug tools for the graph, the implicit product, and the LinearGraph to better understand what is going on

- [x] Investigation: route blowup after adding the full-LinearGraph cut
  - The root cause seems to be the implicit product not being minimized, thus not loosing cut away states, causing path blowup.

- [ ] Idea: Sub SCC, we look at strongly connected subsets of SCCs.
  Probably the easiest would be to look at the path through the SCC (or just the parikh image) if it is reachable, then do a sub-refinement step where we don't disregard the SCC, but instead look if we can remove some edge or node from the SCC to make it unreachable again.

- [ ] Idea: Why are we including the modulo automatons in the LinearGraph?
  1. they do influence SCCs, but we can probably work around that, capturing the relevant nodes with the modulo automatons, but then when translating to a LinearGraph, we can disregard them and get smaller automatons
  2. when solving the LinearGraph, the modulo automatons are strictly weaker than the Z-Reachability we search for, so we can disregard them as well.
  3. When building the automaton in the end we want to construct it in a way that we restrict as much as possible. But the current way we construct them (just take the LinearGraph and invert it) means that by restricting the LinearGraph more, we make the rejected language by the final automaton smaller (and the automaton bigger). This is not great, as yeah, we are more precise, but we already have the precision in the LinearGraph. We are just adding more states to the automaton, which makes it harder to handle.
  - turns out we need some way of writing modulo values to specific SCCs in the LinearGraph. But I think we can encode them better than using automatons. Z3 should allow us to encode these modulo constraints directly.

- [ ] ignore some maybe not all linear graphs. (maybe sleep some linear graphs)

- [ ] find difficult to solve instances
