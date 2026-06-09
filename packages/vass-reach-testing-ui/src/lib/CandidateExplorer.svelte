<script lang="ts">
	import { API_list_light_summaries } from '../fetch';
	import DotGraph from './DotGraph.svelte';

	let { selected } = $props<{ selected: string }>();
	let candidates = $derived(await API_list_light_summaries(selected));
	let selected_name = $state<string | undefined>();
	let candidate = $derived(candidates.find(item => item.instance_name === selected_name) ?? candidates[0]);

	$effect(() => {
		selected;
		selected_name = undefined;
	});
</script>

<div class="container trace-panel">
	<div class="trace-panel-header">
		<h3>Hard Candidates</h3>
		<div class="trace-header-stats">
			<span class="trace-stat-pill">candidates {candidates.length}</span>
		</div>
	</div>
	{#if candidates.length === 0}
		<div>No light candidate summaries found for this test folder.</div>
	{:else}
		<div class="trace-chip-list">
			{#each candidates as item}
				<button
					type="button"
					class:trace-chip={true}
					class:is-selected={candidate?.instance_name === item.instance_name}
					onclick={() => (selected_name = item.instance_name)}
				>
					{item.instance_name}
				</button>
			{/each}
		</div>
		{#if candidate}
			<div class="trace-overview-grid">
				<div class="trace-info-card">
					<div class="trace-card-title">Instance</div>
					<div>counters <strong>{candidate.dimension}</strong></div>
					<div>states <strong>{candidate.state_count}</strong></div>
					<div>transitions <strong>{candidate.transition_count}</strong></div>
				</div>
				<div class="trace-info-card">
					<div class="trace-card-title">Hardness</div>
					<div>reason <strong>{candidate.result_reason}</strong></div>
					<div>steps <strong>[{candidate.step_counts.join(', ')}]</strong></div>
					<div>milliseconds <strong>[{candidate.elapsed_ms.join(', ')}]</strong></div>
				</div>
				<div class="trace-info-card">
					<div class="trace-card-title">Counters</div>
					<div>initial <strong>[{candidate.initial_valuation.join(', ')}]</strong></div>
					<div>target <strong>[{candidate.final_valuation.join(', ')}]</strong></div>
					<div>max update <strong>{candidate.max_update_magnitude}</strong></div>
				</div>
				<div class="trace-info-card">
					<div class="trace-card-title">Provenance</div>
					<div>seed <strong>{candidate.seed}</strong></div>
					<div>repetitions <strong>{candidate.repetitions}</strong></div>
					<div>run <strong>{candidate.run_name}</strong></div>
				</div>
			</div>
			<div class="trace-subsection">
				<div class="trace-card-title">Initial VASS</div>
				<DotGraph dot={candidate.initial_graph_dot} />
			</div>
		{/if}
	{/if}
</div>
