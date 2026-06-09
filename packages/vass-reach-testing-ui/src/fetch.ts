import type { DerivedSCCMetadata, HardCandidateSummary, StepTraceSeed, TestData, TraceRunInfo, TraceStepSccCounterEffectSet, TraceStepSccView } from './types';

declare global {
	interface Window {
		SERVER_PORT: number;
	}
}

function get_server_addr(): string {
	return `http://localhost:${window.SERVER_PORT}`;
}

async function fetch_json<T>(path: string, init: RequestInit): Promise<T> {
	let res = await fetch(`${get_server_addr()}${path}`, init);

	if (!res.ok) {
		let body = '';
		try {
			body = await res.text();
		} catch {
			body = '';
		}

		throw new Error(`Request ${path} failed (${res.status} ${res.statusText})${body ? `: ${body}` : ''}`);
	}

	return (await res.json()) as T;
}

export async function API_list_test_folders(): Promise<string[]> {
	return await fetch_json<string[]>('/api/list_test_folders', {
		method: 'GET',
	});
}

export async function API_test_data(test_folder: string): Promise<TestData> {
	return await fetch_json<TestData>('/api/test_data', {
		method: 'POST',
		body: JSON.stringify(test_folder),
		headers: {
			'Content-Type': 'application/json',
		},
	});
}

export async function API_list_traces(test_folder: string): Promise<TraceRunInfo[]> {
	return await fetch_json<TraceRunInfo[]>('/api/list_traces', {
		method: 'POST',
		body: JSON.stringify(test_folder),
		headers: {
			'Content-Type': 'application/json',
		},
	});
}

export async function API_list_light_summaries(test_folder: string): Promise<HardCandidateSummary[]> {
	return await fetch_json<HardCandidateSummary[]>('/api/list_light_summaries', {
		method: 'POST',
		body: JSON.stringify(test_folder),
		headers: {
			'Content-Type': 'application/json',
		},
	});
}

export async function API_list_trace_steps(test_folder: string, run_name: string, instance_name: string): Promise<number[]> {
	return await fetch_json<number[]>('/api/list_trace_steps', {
		method: 'POST',
		body: JSON.stringify({
			folder: test_folder,
			run_name,
			instance_name,
		}),
		headers: {
			'Content-Type': 'application/json',
		},
	});
}

export async function API_trace_step_seed(test_folder: string, run_name: string, instance_name: string, step: number): Promise<StepTraceSeed> {
	return await fetch_json<StepTraceSeed>('/api/trace_step_seed', {
		method: 'POST',
		body: JSON.stringify({
			folder: test_folder,
			run_name,
			instance_name,
			step,
		}),
		headers: {
			'Content-Type': 'application/json',
		},
	});
}

export async function API_trace_step_metadata(test_folder: string, run_name: string, instance_name: string, step: number): Promise<DerivedSCCMetadata> {
	return await fetch_json<DerivedSCCMetadata>('/api/trace_step_metadata', {
		method: 'POST',
		body: JSON.stringify({
			folder: test_folder,
			run_name,
			instance_name,
			step,
		}),
		headers: {
			'Content-Type': 'application/json',
		},
	});
}

export async function API_trace_step_scc_view(test_folder: string, run_name: string, instance_name: string, step: number, component_index: number): Promise<TraceStepSccView> {
	return await fetch_json<TraceStepSccView>('/api/trace_step_scc_view', {
		method: 'POST',
		body: JSON.stringify({
			folder: test_folder,
			run_name,
			instance_name,
			step,
			component_index,
		}),
		headers: {
			'Content-Type': 'application/json',
		},
	});
}

export async function API_trace_step_scc_counter_effects(
	test_folder: string,
	run_name: string,
	instance_name: string,
	step: number,
	component_index: number,
	entry_state: number[],
	start_value: number,
): Promise<TraceStepSccCounterEffectSet> {
	if (!Array.isArray(entry_state)) {
		throw new Error('Missing required entry_state for /api/trace_step_scc_counter_effects request.');
	}

	return await fetch_json<TraceStepSccCounterEffectSet>('/api/trace_step_scc_counter_effects', {
		method: 'POST',
		body: JSON.stringify({
			folder: test_folder,
			run_name,
			instance_name,
			step,
			component_index,
			entry_state,
			start_value,
		}),
		headers: {
			'Content-Type': 'application/json',
		},
	});
}
