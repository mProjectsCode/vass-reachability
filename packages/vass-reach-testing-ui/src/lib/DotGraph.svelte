<script lang="ts">
	import { instance, type Viz } from '@viz-js/viz';

	let { dot } = $props<{ dot: string }>();

	let container = $state<HTMLDivElement | undefined>(undefined);
	let render_error = $state<string | undefined>(undefined);
	let render_version = 0;

	let viz_instance_promise: Promise<Viz> | undefined;

	async function get_viz_instance(): Promise<Viz> {
		if (!viz_instance_promise) {
			viz_instance_promise = instance();
		}
		return viz_instance_promise;
	}

	async function render_dot_graph(dot_source: string, target: HTMLDivElement): Promise<void> {
		const current_render_version = ++render_version;
		render_error = undefined;

		try {
			const viz = await get_viz_instance();
			if (current_render_version !== render_version) {
				return;
			}

			const svg = viz.renderSVGElement(dot_source);
			if (current_render_version !== render_version) {
				return;
			}

			svg.style.display = 'block';
			svg.style.width = '100%';
			svg.style.minWidth = '100%';
			svg.style.height = 'auto';
			svg.style.maxWidth = 'none';
			target.replaceChildren(svg);
		} catch (error) {
			if (current_render_version !== render_version) {
				return;
			}

			render_error = error instanceof Error ? error.message : String(error);
			target.replaceChildren();
		}
	}

	$effect(() => {
		if (!container || !dot) {
			return;
		}

		void render_dot_graph(dot, container);
	});
</script>

<div class="dot-graph" bind:this={container}></div>
{#if render_error}
	<div class="dot-error">Failed to render DOT graph: {render_error}</div>
{/if}
