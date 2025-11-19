export enum PlotDatumState {
    True = "True",
    False = "False",
    Unknown = "Unknown",
    Conflict = "Conflict",
    Crash = "Crash"
}

export function map_datum_state(existing: PlotDatumState | undefined, state: SolverRunResult): PlotDatumState {
    let new_state: PlotDatumState;
    if ("Success" in state) {
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

export type PlotDatum = {
    state: PlotDatumState;
    times: Record<string, number>;
    net: string;
};

export interface TestData {
    path: string,
    instance_config: InstanceConfig,
    test_config: TestConfig,
    tool_results: ToolResult[],
}

export interface InstanceConfig {
    num_instances: number,
    seed: number,
    petri_net_counters: number,
    petri_net_transitions: number,
    petri_net_max_tokens_per_transition: number,
    petri_net_no_guards: boolean,
}

export interface TestConfig {
    tools: string[],
    timeout: number,
    memory_max_gb: number,
}

export interface ToolResult {
    tool_name: string,
    results: Record<string, SolverResultStatistic>,
}

export interface SolverResultStatistic {
    result: SolverRunResult,
    ms_taken: number,
}

export type SolverRunResult = {
    "Success": {
        status: "True" | "False" | "Unknown",
        statistics: null,
    }
} | {
    "Crash": string
}

