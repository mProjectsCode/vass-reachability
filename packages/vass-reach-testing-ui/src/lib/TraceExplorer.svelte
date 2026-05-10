<script lang="ts">
	import { API_list_trace_steps, API_list_traces, API_trace_step_scc_counter_effects, API_trace_step_scc_view, API_trace_step_metadata, API_trace_step_seed } from '../fetch';
	import type { TraceRunInfo } from '../types';
	import DotGraph from './DotGraph.svelte';

	let { selected } = $props<{ selected: string }>();

	let traces = $derived((await API_list_traces(selected)) as TraceRunInfo[]);
	let selected_trace_run = $state<string | undefined>();
	let selected_trace_instance = $state<string | undefined>();
	let selected_trace_step = $state<number | undefined>();
	let available_steps = $derived(
		selected_trace_run && selected_trace_instance ? await API_list_trace_steps(selected, selected_trace_run, selected_trace_instance) : ([] as number[]),
	);
	let selected_step_seed = $derived(
		selected_trace_run && selected_trace_instance && selected_trace_step !== undefined
			? await API_trace_step_seed(selected, selected_trace_run, selected_trace_instance, selected_trace_step)
			: undefined,
	);
	let selected_step_metadata = $derived(
		selected_trace_run && selected_trace_instance && selected_trace_step !== undefined
			? await API_trace_step_metadata(selected, selected_trace_run, selected_trace_instance, selected_trace_step)
			: undefined,
	);
	let selected_scc_component = $state<number | undefined>();

	let selected_scc_view = $derived(
		selected_trace_run && selected_trace_instance && selected_trace_step !== undefined && selected_scc_component !== undefined
			? await API_trace_step_scc_view(selected, selected_trace_run, selected_trace_instance, selected_trace_step, selected_scc_component)
			: undefined,
	);
	let selected_scc_entry_key = $state<string | undefined>();
	let scc_start_value = $state<number>(0);

	type BoundaryState = { key: string; state: number[] };

	function state_key(state: number[]): string {
		return state.join(',');
	}

	let selected_scc_entries = $derived<BoundaryState[]>((selected_scc_view?.entries ?? []).map(state => ({ key: state_key(state), state })));
	let selected_scc_exits = $derived<BoundaryState[]>((selected_scc_view?.exits ?? []).map(state => ({ key: state_key(state), state })));
	let selected_scc_entry = $derived(selected_scc_entries.find(entry => entry.key === selected_scc_entry_key));
	let selected_scc_view_is_current = $derived(selected_scc_component !== undefined && selected_scc_view?.component_index === selected_scc_component);
	let selected_scc_entry_state = $derived(Array.isArray(selected_scc_entry?.state) ? selected_scc_entry.state : undefined);
	let selected_scc_counter_effects = $derived(
		selected_trace_run &&
			selected_trace_instance &&
			selected_trace_step !== undefined &&
			selected_scc_component !== undefined &&
			selected_scc_view_is_current &&
			selected_scc_entry_state
			? await API_trace_step_scc_counter_effects(
					selected,
					selected_trace_run,
					selected_trace_instance,
					selected_trace_step,
					selected_scc_component,
					selected_scc_entry_state,
					Math.trunc(scc_start_value),
				)
			: undefined,
	);

	let selected_run_info = $derived(traces.find(run => run.run_name === selected_trace_run));

	let trace_overview = $derived.by(() => {
		if (!selected_step_seed || !selected_step_metadata) {
			return undefined;
		}

		const component_sizes = selected_step_metadata.component_sizes;
		const accepting_sizes = selected_step_metadata.accepting_sizes;
		const cyclic_components = selected_step_metadata.cyclic_components;

		const cyclic_count = cyclic_components.filter(Boolean).length;
		const accepting_component_count = accepting_sizes.filter(size => size > 0).length;

		const min_component_size = component_sizes.length ? Math.min(...component_sizes) : 0;
		const max_component_size = component_sizes.length ? Math.max(...component_sizes) : 0;
		const avg_component_size = component_sizes.length ? component_sizes.reduce((sum, size) => sum + size, 0) / component_sizes.length : 0;

		return {
			transition_count: selected_step_seed.found_path.path.transitions.length,
			state_count: selected_step_seed.found_path.path.states.length,
			component_count: selected_step_seed.scc_dag.components.length,
			edge_count: selected_step_seed.scc_dag.edges.reduce((sum, edges) => sum + edges.length, 0),
			cyclic_count,
			accepting_component_count,
			min_component_size,
			max_component_size,
			avg_component_size,
		};
	});

	let selected_step_index = $derived(selected_trace_step === undefined ? -1 : available_steps.findIndex(step => step === selected_trace_step));

	function format_vector(values: number[]): string {
		return `[${values.join(', ')}]`;
	}

	$effect(() => {
		selected;
		selected_trace_run = undefined;
		selected_trace_instance = undefined;
		selected_trace_step = undefined;
		selected_scc_component = undefined;
	});

	$effect(() => {
		const runs = traces;
		if (!runs.length) {
			selected_trace_run = undefined;
			selected_trace_instance = undefined;
			selected_trace_step = undefined;
			return;
		}
		if (!selected_trace_run || !runs.some(run => run.run_name === selected_trace_run)) {
			selected_trace_run = runs[0].run_name;
			selected_trace_instance = runs[0].instances[0];
			selected_trace_step = undefined;
			return;
		}

		const current_run = runs.find(run => run.run_name === selected_trace_run);
		if (!current_run || !current_run.instances.length) {
			selected_trace_instance = undefined;
			selected_trace_step = undefined;
			return;
		}

		if (!selected_trace_instance || !current_run.instances.includes(selected_trace_instance)) {
			selected_trace_instance = current_run.instances[0];
			selected_trace_step = undefined;
		}
	});

	$effect(() => {
		const steps = available_steps;
		if (!steps.length) {
			selected_trace_step = undefined;
			return;
		}

		if (selected_trace_step === undefined || !steps.includes(selected_trace_step)) {
			selected_trace_step = steps[0];
		}
	});

	$effect(() => {
		const seed = selected_step_seed;
		if (!seed) {
			selected_scc_component = undefined;
			selected_scc_entry_key = undefined;
			return;
		}

		if (selected_scc_component === undefined || selected_scc_component >= seed.scc_dag.components.length) {
			selected_scc_component = seed.scc_dag.root_component;
		}
	});

	$effect(() => {
		const component = selected_scc_component;
		component;
		selected_scc_entry_key = undefined;
	});

	$effect(() => {
		const entries = selected_scc_entries;
		if (entries.length === 0) {
			selected_scc_entry_key = undefined;
			return;
		}

		if (!selected_scc_entry_key || !entries.some(entry => entry.key === selected_scc_entry_key)) {
			selected_scc_entry_key = entries[0].key;
		}
	});
</script>

<div class="container trace-panel">
	<div class="trace-panel-header">
		<h3>Trace Explorer</h3>
		<div class="trace-header-stats">
			<span class="trace-stat-pill">runs {traces.length}</span>
			<span class="trace-stat-pill">steps {available_steps.length}</span>
		</div>
	</div>
	{#if traces.length === 0}
		<div>No traces found for this test folder.</div>
	{:else}
		<div class="trace-selector-grid">
			<div class="trace-selector-row">
				<span class="trace-selector-label">Run</span>
				<div class="trace-chip-list">
					{#each traces as run}
						<button
							type="button"
							class:trace-chip={true}
							class:is-selected={selected_trace_run === run.run_name}
							onclick={() => {
								selected_trace_run = run.run_name;
								selected_trace_instance = run.instances[0];
								selected_trace_step = undefined;
							}}
						>
							{run.run_name}
							<small>({run.instances.length})</small>
						</button>
					{/each}
				</div>
			</div>

			{#if selected_trace_run}
				<div class="trace-selector-row">
					<span class="trace-selector-label">Instance</span>
					<div class="trace-chip-list">
						{#each selected_run_info?.instances ?? [] as instance_name}
							<button
								type="button"
								class:trace-chip={true}
								class:is-selected={selected_trace_instance === instance_name}
								onclick={() => {
									selected_trace_instance = instance_name;
									selected_trace_step = undefined;
								}}
							>
								{instance_name}
							</button>
						{/each}
					</div>
				</div>
			{/if}

			{#if selected_trace_instance}
				<div class="trace-selector-row">
					<span class="trace-selector-label">Step</span>
					<div class="trace-chip-list trace-step-list">
						<button
							type="button"
							class="trace-nav-button"
							disabled={selected_step_index <= 0}
							onclick={() => {
								if (selected_step_index > 0) {
									selected_trace_step = available_steps[selected_step_index - 1];
								}
							}}
						>
							Prev
						</button>
						{#each available_steps as step}
							<button
								type="button"
								class:trace-chip={true}
								class:is-selected={selected_trace_step === step}
								onclick={() => {
									selected_trace_step = step;
								}}
							>
								{step}
							</button>
						{/each}
						<button
							type="button"
							class="trace-nav-button"
							disabled={selected_step_index < 0 || selected_step_index >= available_steps.length - 1}
							onclick={() => {
								if (selected_step_index >= 0 && selected_step_index < available_steps.length - 1) {
									selected_trace_step = available_steps[selected_step_index + 1];
								}
							}}
						>
							Next
						</button>
					</div>
				</div>
			{/if}
		</div>

		{#if selected_step_seed && selected_step_metadata}
			<div class="trace-overview-grid">
				<div class="trace-info-card">
					<div class="trace-card-title">Selection</div>
					<div>run <strong>{selected_trace_run}</strong></div>
					<div>instance <strong>{selected_trace_instance}</strong></div>
					<div>
						step <strong>{selected_step_seed.step}</strong>
						({selected_step_index + 1}/{available_steps.length})
					</div>
				</div>
				<div class="trace-info-card">
					<div class="trace-card-title">Path</div>
					<div>n-reaching <strong>{selected_step_seed.found_path.n_reaching ? 'true' : 'false'}</strong></div>
					<div>states <strong>{trace_overview?.state_count}</strong></div>
					<div>transitions <strong>{trace_overview?.transition_count}</strong></div>
				</div>
				<div class="trace-info-card">
					<div class="trace-card-title">SCC DAG</div>
					<div>components <strong>{trace_overview?.component_count}</strong></div>
					<div>edges <strong>{trace_overview?.edge_count}</strong></div>
					<div>cyclic components <strong>{trace_overview?.cyclic_count}</strong></div>
					<div>accepting components <strong>{trace_overview?.accepting_component_count}</strong></div>
				</div>
				<div class="trace-info-card">
					<div class="trace-card-title">Component Sizes</div>
					<div>min <strong>{trace_overview?.min_component_size}</strong></div>
					<div>max <strong>{trace_overview?.max_component_size}</strong></div>
					<div>avg <strong>{trace_overview ? trace_overview.avg_component_size.toFixed(2) : '0.00'}</strong></div>
					<div class="trace-muted">[{selected_step_metadata.component_sizes.join(', ')}]</div>
				</div>
			</div>

			<div class="dot-viewer">
				<div class="trace-card-title">SCC DAG</div>
				<DotGraph dot={selected_step_seed.scc_dag.dot} />
			</div>

			<div class="trace-subsection">
				<div class="trace-card-title">SCC Component Explorer</div>
				<div class="trace-selector-row">
					<span class="trace-selector-label">Component</span>
					<div class="trace-chip-list">
						{#each selected_step_seed.scc_dag.components as component, component_index}
							<button
								type="button"
								class:trace-chip={true}
								class:is-selected={selected_scc_component === component_index}
								onclick={() => {
									selected_scc_component = component_index;
								}}
							>
								SCC {component_index}
								<small>{component.nodes.length} nodes</small>
								{#if component_index === selected_step_seed.scc_dag.root_component}
									<small>root</small>
								{/if}
								{#if component.cyclic}
									<small>cyclic</small>
								{/if}
							</button>
						{/each}
					</div>
				</div>

				{#if selected_scc_view}
					<div class="trace-overview-compact">
						<div class="trace-inline-summary">
							<span class="trace-stat-pill">entries {selected_scc_view.entries.length}</span>
							<span class="trace-stat-pill">exits {selected_scc_view.exits.length}</span>
							<span class="trace-stat-pill">effect classes {selected_scc_counter_effects?.effect_set.length ?? 0}</span>
							{#if selected_scc_component === selected_step_seed.scc_dag.root_component}
								<span class="trace-stat-pill">includes root entry</span>
							{/if}
						</div>

						<div class="trace-boundary-grid">
							<div class="trace-info-card">
								<div class="trace-card-title">Entries</div>
								<div class="trace-boundary-list">
									{#if selected_scc_entries.length === 0}
										<span class="trace-muted">none</span>
									{:else}
										{#each selected_scc_entries as entry}
											<span class="trace-boundary-chip entry">{entry.key}</span>
										{/each}
									{/if}
								</div>
							</div>
							<div class="trace-info-card">
								<div class="trace-card-title">Exits</div>
								<div class="trace-boundary-list">
									{#if selected_scc_exits.length === 0}
										<span class="trace-muted">none</span>
									{:else}
										{#each selected_scc_exits as exit}
											<span class="trace-boundary-chip exit">{exit.key}</span>
										{/each}
									{/if}
								</div>
							</div>
						</div>

						<div class="trace-selector-row">
							<span class="trace-selector-label">Entry For Counter Effects</span>
							<div class="trace-chip-list">
								{#if selected_scc_entries.length === 0}
									<span class="trace-muted">No entry node available in this SCC.</span>
								{:else}
									{#each selected_scc_entries as entry}
										<button
											type="button"
											class:trace-chip={true}
											class:is-selected={selected_scc_entry_key === entry.key}
											onclick={() => {
												selected_scc_entry_key = entry.key;
											}}
										>
											{entry.key}
										</button>
									{/each}
								{/if}
							</div>
						</div>

						<div class="trace-selector-row">
							<span class="trace-selector-label">Start Counter Value (all counters)</span>
							<div class="trace-chip-list">
								<input type="number" bind:value={scc_start_value} step="1" />
								<span class="trace-muted">only cycles that stay non-negative are kept</span>
							</div>
						</div>

						<div class="trace-info-card">
							<div class="trace-card-title">Counter Effect Set</div>
							{#if !selected_scc_entry}
								<div class="trace-muted">Select an entry node to derive rooted basic cycles.</div>
							{:else if !selected_scc_counter_effects}
								<div class="trace-muted">Loading counter effects...</div>
							{:else}
								<div>entry <strong>{format_vector(selected_scc_counter_effects.entry)}</strong></div>
								<div>start value <strong>{selected_scc_counter_effects.start_value}</strong></div>
								<div>dimension <strong>{selected_scc_counter_effects.dimension}</strong></div>
								<div>basic cycles <strong>{selected_scc_counter_effects.total_cycles}</strong></div>
								<div>effect classes <strong>{selected_scc_counter_effects.effect_set.length}</strong></div>
								{#if selected_scc_counter_effects.capped}
									<div class="trace-muted">Cycle enumeration capped for readability and performance.</div>
								{/if}

								{#if selected_scc_counter_effects.effect_set.length === 0}
									<div class="trace-muted">No rooted cycle effect found from this entry.</div>
								{:else}
									<div class="trace-effect-list">
										{#each selected_scc_counter_effects.effect_set as effect}
											<div class="trace-effect-row">
												<div>effect <strong>{format_vector(effect.effect)}</strong></div>
												<div class="trace-muted">sample cycle length {effect.sample_cycle.transitions.length}</div>
											</div>
										{/each}
									</div>
								{/if}
							{/if}
						</div>
					</div>

					<div class="dot-viewer">
						<div class="trace-card-title">Selected SCC Graph</div>
						<DotGraph dot={selected_scc_view.dot} />
					</div>
				{/if}
			</div>
		{/if}
	{/if}
</div>
