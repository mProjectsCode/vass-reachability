This document outlines the idea behind the reachability algorithm implemented by this tool.

This document only focuses on zero reachability, meaning that the initial and final counter valuations are zero, since for every instance of the VASS reachability problem a zero reachability instance can be created that is reachable iff. the VASS reachability instance is reachable. 

Given an initialized VASS $V$ with initial and final counter valuations 0, we want to solve $L(V) = \emptyset$, where $L(V)$ is the reachability language of the initialized VASS. Due to recent results [On the Separability Problem of VASS Reachability Languages](http://arxiv.org/pdf/2401.16095), the following holds.

$$
L(V) = \emptyset \\
\text{iff.} \quad L(CFG_V) \cap L(\text{Dyck}) = \emptyset \\
\text{iff.} \quad L(CFG_V)\ |\ L(\text{Dyck})
$$

To show $L(CFG_V)\ |\ L(\text{Dyck})$, this tool attempts to incrementally construct a DFA $A$ which by construction is disjoint from $L(\text{Dyck})$ and which includes $L(CFG_V)$. In reality $A$ is a union over a set of DFA $\mathcal{A}$.

A modulo DFA tracks a the counters of $V$ modulo $\mu \in \mathbb{N}$. It starts at the state where all counters are $0\ \text{mod}\ \mu$ and it accepts in any state other than the start state. $\text{MDFA}_\mu$ refers to the modulo DFA.

## The Algorithm

Maybe: Check for Z-VASS reachability and return `false` if it is not reachable.

The tool starts with $\mathcal{A} = \{\text{MDFA}_2\}$. 

It repeatedly constructs $A$ from $\mathcal{A}$, then inverts $A$ and builds the intersection automaton of $A$ and $CFG_V$. 

### Case 1: The intersection automation does not contain an accepting state

The tool returns `false`.

### Case 2: The intersection automaton contains an accepting state

The tool does a BFS for an accepting state. Thus it finds the shortest path to an accepting state. Then the tool checks the labels along the path an simulates their effect on the counters.

#### Sub-Case 2.1: The counters are all 0 at the end of the path

We have a reaching path. The tool returns `true`.

#### Sub-Case 2.2: None of the counter values have been bigger or equal to $\mu$ and a counter value becomes negative

A bigger $\mu$ for the MDFA will not help here, so we add a new automaton to $\mathcal{A}$ that has states that follow the path and at the position where a counter value becomes negative it accepts and stays in that accepting state. The language of this new DFA is disjoint from Dyck, since it accepts only paths that have a negative intermediate counter valuation.

#### Sub-Case 2.3: The path is only accepted because it runs a loop and wraps to achieve a low counter valuation

A bigger $\mu$ for the MDFA will not help here, since the path can simply run the loop more often and still wrap a counter.

TODO: Is this even possible when the VASS has a Z solution?

#### Sub-Case 2.4: Else

We increase $\mu$.