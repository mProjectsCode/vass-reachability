<script lang="ts">
    import { Dot, Plot } from "svelteplot";
    import { map_state_to_color, PlotDatumState, type PlotDatum, type TestData } from "../types";

    let {
        plot_data,
        selected,
    }: {
        plot_data: PlotDatum[],
        selected: TestData,
    } = $props();

    let compare_run_x: string | undefined = $state();
    let compare_run_y: string | undefined = $state();
    let filter_crashes: boolean = $state(false);
    let sync_bounds: boolean = $state(false);
    let plot_container_width: number = $state(0);
    let plot_container_height: number = $state(0);

    let filtered_plot_data = $derived.by(() => {
        let filtered = plot_data;

        if (filter_crashes) {
            filtered = filtered.filter(x => x.state !== PlotDatumState.Crash);
        }

        return filtered;
    });

    const DEFAULT_BOUNDS  = {
        min_x: 1,
        min_y: 1,
        max_x: 10,
        max_y: 10,
    };

    let {
        min_x, min_y, max_x, max_y
    } = $derived.by(() => {
        if (filtered_plot_data.length === 0 || !compare_run_x || !compare_run_y) {
            return DEFAULT_BOUNDS;
        }

        let bounds = {
            min_x: Number.MAX_VALUE,
            min_y: Number.MAX_VALUE,
            max_x: 0,
            max_y: 0,
        };

        for (const datum of filtered_plot_data) {
            if (datum.times[compare_run_x] < bounds.min_x) {
                bounds.min_x = datum.times[compare_run_x];
            }
            if (datum.times[compare_run_y] < bounds.min_y) {
                bounds.min_y = datum.times[compare_run_y];
            }

            if (datum.times[compare_run_x] > bounds.max_x) {
                bounds.max_x = datum.times[compare_run_x];
            }
            if (datum.times[compare_run_y] > bounds.max_y) {
                bounds.max_y = datum.times[compare_run_y];
            }
        }

        if (sync_bounds) {
            let min = Math.min(bounds.min_x, bounds.min_y);
            let max = Math.max(bounds.max_x, bounds.max_y);
            bounds.min_x = min;
            bounds.min_y = min;
            bounds.max_x = max;
            bounds.max_y = max;
        }

        return bounds;
    });
</script>

<div class="container flex-row">
    <span>Run on X axis:</span>
    {#each selected.test_config.runs as run}
        <button type="button" class:is-selected={compare_run_x === run.name} onclick={() => { compare_run_x = run.name }}>
            {run.name}
        </button>
    {/each}
</div>
<div class="container flex-row">
    <span>Run on Y axis:</span>
    {#each selected.test_config.runs as run}
        <button type="button" class:is-selected={compare_run_y === run.name} onclick={() => { compare_run_y = run.name }}>
            {run.name}
        </button>
    {/each}
</div>
<div class="container flex-row">
    <button type="button" class:is-selected={filter_crashes} onclick={() => { filter_crashes = !filter_crashes }}>
        filter crashes
    </button>
    <button type="button" class:is-selected={sync_bounds} onclick={() => { sync_bounds = !sync_bounds }}>
        sync bounds
    </button>
</div>
<div class="container flex-row">
    {#each Object.values(PlotDatumState) as state}
        <span><div class="color-indicator" style="background-color: {map_state_to_color(state)};"></div>{state}</span>
    {/each}
</div>
<div class="container plot-container" bind:clientWidth={plot_container_width} bind:clientHeight={plot_container_height}>
    {#if compare_run_x && compare_run_y && plot_container_width && plot_container_height}
        <Plot 
            axes 
            grid 
            x={{ type: "log", domain: [min_x, max_x] }} 
            y={{ type: "log", domain: [min_y, max_y] }}
            width={plot_container_width}
            height={plot_container_height}
        >
            <Dot data={filtered_plot_data} x={(d) => d.times[compare_run_x!]} y={(d) => d.times[compare_run_y!]} stroke={(d) => map_state_to_color(d.state)} />
        </Plot>
    {:else}
        <span>
            Please select tools to display above.
        </span>
    {/if}
</div>