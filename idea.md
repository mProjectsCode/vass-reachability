This document outlines the idea behind the reachability algorithm implemented by this tool.

Given an initialized VASS $V$ with initial and final counter valuations, we want to solve $L(V) = \emptyset$, where $L(V)$ is the reachability language of the initialized VASS. Due to recent results [On the Separability Problem of VASS Reachability Languages](http://arxiv.org/pdf/2401.16095), the following holds.

$$
L(V) = \emptyset \\
\text{iff.} \quad L(CFG_V) \cap L(\text{Dyck}) = \emptyset \\
\text{iff.} \quad L(CFG_V)\ |\ L(\text{Dyck})
$$

To show $L(CFG_V)\ |\ L(\text{Dyck})$, this tool attempts to incrementally construct a DFA $A$ which by construction is disjoint from $L(\text{Dyck})$ and which includes $L(CFG_V)$. In reality we only approximate $L(\text{Dyck})$ with a modulo DFA and cut false positives out of $CFG_V$.

A modulo DFA tracks a the counters of $V$ modulo $\mu \in \mathbb{N}$. It starts at the state where all counters are $0\ \text{mod}\ \mu$ and it accepts in any state other than the start state. $\text{MDFA}_\mu$ refers to the modulo DFA.

## The Algorithm

Maybe: Check for Z-VASS reachability and return `false` if it is not reachable.

The tool starts with $\mu = 2$ and does a BFS on $CFG_V$ which also respects the counter values. The result is a path through the $CFG_V$ that is modulo $\mu$ accepting, if such a path exists.

### Case 1: No such path exists

The tool returns `false`.

### Case 2: A modulo $\mu$ reaching path exists

The tool checks the properties of this path.

#### Sub-Case 2.1: The path is actually reaching

We have a reaching path. The tool returns `true`.

#### Sub-Case 2.2: The path has a loop

TODO

#### Sub-Case 2.3: The path does not stay positive

TODO

#### Sub-Case 2.4: Else

We increase $\mu$.