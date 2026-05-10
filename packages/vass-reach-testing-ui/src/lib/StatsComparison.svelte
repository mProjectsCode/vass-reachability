<script lang="ts">
	import type { PlotDatum } from '../types';

	let {
		plot_data,
		run_x,
		run_y,
	}: {
		plot_data: PlotDatum[];
		run_x: string;
		run_y: string;
	} = $props();

	function is_non_success(result: { [key: string]: unknown }): boolean {
		return 'Crash' in result || 'OOM' in result || 'Timeout' in result;
	}

	let without_crashes = $derived(
		plot_data.filter(x => !is_non_success(x.runs[run_x].result as { [key: string]: unknown }) && !is_non_success(x.runs[run_y].result as { [key: string]: unknown })),
	);

	let average_speedup = $derived.by(() => {
		let speedup = 0;
		for (const datum of without_crashes) {
			let time_x = datum.runs[run_x].ms_taken;
			let time_y = datum.runs[run_y].ms_taken;
			speedup += time_x / time_y;
		}
		return speedup / without_crashes.length;
	});

	let x_faster_count = $derived.by(() => {
		let count = 0;
		for (const datum of without_crashes) {
			let time_x = datum.runs[run_x].ms_taken;
			let time_y = datum.runs[run_y].ms_taken;
			if (time_x / time_y < 1.05) {
				count += 1;
			}
		}
		return count;
	});
</script>

<div class="container">runsruns</div>
