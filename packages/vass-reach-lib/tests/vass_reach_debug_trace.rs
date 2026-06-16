use std::time::{SystemTime, UNIX_EPOCH};

use vass_reach_lib::{
    automaton::{
        ModifiableAutomaton,
        vass::{VASS, VASSEdge},
    },
    config::{DebugTraceConfig, DebugTraceLevel, VASSReachConfig},
    solver::vass_reach::VASSReachSolver,
};

#[test]
fn light_trace_writes_summary_without_steps() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("vass-light-trace-{unique}"));
    let mut vass = VASS::<usize, usize>::new(1, vec![0]);
    let node = vass.add_node(0);
    vass.add_edge(&node, &node, VASSEdge::new(0, vec![1].into()));
    let instance = vass.init(vec![0].into(), vec![0].into(), node, node);
    let trace = DebugTraceConfig::default()
        .with_enabled(true)
        .with_level(DebugTraceLevel::Light)
        .with_output_root(Some(root.display().to_string()))
        .with_run_name(Some("test-run".to_string()))
        .with_instance_name(Some("test-instance".to_string()));

    let _ = VASSReachSolver::new(
        &instance,
        VASSReachConfig::default().with_debug_trace(trace),
    )
    .solve();

    let instance_dir = root.join("test-run").join("test-instance");
    assert!(instance_dir.join("summary.json").is_file());
    assert!(!instance_dir.join("steps").exists());
    let _ = std::fs::remove_dir_all(root);
}
