export enum PlotDatumState {
	True = 'True',
	False = 'False',
	Unknown = 'Unknown',
	Conflict = 'Conflict',
	Crash = 'Crash',
}

export function map_datum_state(existing: PlotDatumState | undefined, state: SolverRunResult): PlotDatumState {
	let new_state: PlotDatumState;
	if ('Success' in state) {
		new_state = state.Success.status as PlotDatumState;
	} else {
		new_state = PlotDatumState.Crash;
	}

	if (existing === undefined) {
		return new_state;
	}

	if (existing === PlotDatumState.Crash || new_state === PlotDatumState.Crash) {
		return PlotDatumState.Crash;
	}

	if (existing !== new_state) {
		return PlotDatumState.Conflict;
	}

	return existing;
}

const STATE_COLOR_MAP: Record<PlotDatumState, string> = {
	[PlotDatumState.True]: '#6EEB8D',
	[PlotDatumState.False]: '#EB6E77',
	[PlotDatumState.Unknown]: '#EBCE6E',
	[PlotDatumState.Conflict]: '#EB66E1',
	[PlotDatumState.Crash]: '#6E7EEB',
};

export function map_state_to_color(state: PlotDatumState): string {
	return STATE_COLOR_MAP[state];
}

export type PlotDatum = {
	state: PlotDatumState;
	runs: Record<string, SolverResultStatistic>;
	net: string;
};

export interface TestData {
	path: string;
	instance_config: InstanceConfig;
	test_config: TestConfig;
	tool_results: ToolResult[];
}

export interface InstanceConfig {
	num_instances: number;
	seed: number;
	petri_net_counters: number;
	petri_net_transitions: number;
	petri_net_max_tokens_per_transition: number;
	petri_net_no_guards: boolean;
}

export interface TestConfig {
	runs: TestRunConfig[];
	timeout: number;
	memory_max_gb: number;
}

export interface TestRunConfig {
	name: string;
	tool: string;
	config: string;
}

export interface ToolResult {
	tool: string;
	run_name: string;
	results: Record<string, SolverResultStatistic>;
}

export interface SolverResultStatistic {
	result: SolverRunResult;
	ms_taken: number;
}

export type SolverRunResult =
	| {
			Success: {
				status: 'True' | 'False' | 'Unknown';
				statistics: null;
			};
	  }
	| {
			Crash: string;
	  };

export interface TraceRunInfo {
	run_name: string;
	instances: string[];
}

export interface PathSeed {
	states: number[][];
	transitions: string[];
}

export interface FoundPathSeed {
	n_reaching: boolean;
	path: PathSeed;
}

export interface SCCComponentSeed {
	cyclic: boolean;
	nodes: number[][];
	accepting_nodes: number[][];
	internal_edges: SCCComponentEdgeSeed[];
}

export interface SCCComponentEdgeSeed {
	source: number[][];
	target: number[][];
	transition: string;
}

export interface SCCDagEdgeSeed {
	target_component: number;
	path: PathSeed;
}

export interface SCCDagSeed {
	root_component: number;
	trivial_paths_rolled: boolean;
	dot: string;
	components: SCCComponentSeed[];
	edges: SCCDagEdgeSeed[][];
}

export interface StepTraceSeed {
	schema_version: number;
	step: number;
	found_path: FoundPathSeed;
	scc_dag: SCCDagSeed;
}

export interface DerivedSCCMetadata {
	component_sizes: number[];
	accepting_sizes: number[];
	cyclic_components: boolean[];
}

export interface BoundaryState {
	key: string;
	state: number[];
}

export interface TraceStepSccView {
	component_index: number;
	dot: string;
	entries: BoundaryState[];
	exits: BoundaryState[];
}
