# VASS Reach Testing

This crate generates instances, runs solver benchmarks, finds small instances
that are difficult for the current solver, minimizes selected candidates, and
serves the testing UI.

Commands below are run from the workspace root.

## Hard-Instance Search

The scaffolded search folder is:

```text
packages/vass-reach-testing/test_data/hard_instance_search
```

Run a release-mode search with:

```sh
cargo run --release -p vass-reach-testing -- \
  packages/vass-reach-testing/test_data/hard_instance_search \
  --mode search
```

Search settings live in `instances.toml`. The default ranges cover 2–4
counters, 1–4 states, 3–10 transitions, updates from -3 to 3, and initial and
target values from 0 to 3.

The search runs the solver directly with bounded counting and preprocessing
disabled. An instance is retained only when every configured repetition reaches
the timeout or iteration limit.

Outputs:

- `instances/*.vass.json`: retained candidates.
- `light-traces/vass-reach/hard-instance-search/*/summary.json`: compact
  candidate facts and the initial VASS as Graphviz DOT.
- `search-results.json`: aggregate results ranked by structural size.

Search mode does not write full per-step solver traces.

## Testing Candidates

Run all instances currently in the scaffolded folder through the normal testing
harness:

```sh
cd packages/vass-reach-testing
cargo run --release -- test_data/hard_instance_search --mode test
```

The run configuration in `vass_reach_hard_instance.toml` matches the search
baseline. Keep `debug_trace.enabled = false` for bulk runs.

For a deliberate rerun of a selected candidate, debug tracing supports:

```toml
[debug_trace]
enabled = true
level = "light" # or "full"
```

Light mode writes one `summary.json` and no step files. Full mode writes the
existing per-step path and SCC artifacts and should only be used for a small
number of selected candidates.

## Representative Unreachable Candidates

The curated selection in
`test_data/hard_instance_search/representative-unreachable.json` contains four
small unreachable instances and one alternate. All selected instances have two
counters and two states; the smallest has four transitions and the others have
five.

They were verified with `vass_reach_verification.toml`, which enables bounded
counting and preprocessing. The two ranking-certified entries additionally
have finite reachable configuration spaces and were checked by exhaustive
exploration.

## Visualization

Start the testing UI from this crate:

```sh
cd packages/vass-reach-testing
cargo run --release -- --mode visualize
```

Select `hard_instance_search` in the UI. The Hard Candidates panel reads the
compact search summaries and renders each initial VASS. The Trace Explorer
continues to display full traces when they exist.

## Random Generation

The existing `generate` mode still creates Petri nets by default. Set
`generate_vass = true` in `instances.toml` to write ranged general VASS
instances instead:

```sh
cargo run --release -p vass-reach-testing -- path/to/test-folder --mode generate
```
