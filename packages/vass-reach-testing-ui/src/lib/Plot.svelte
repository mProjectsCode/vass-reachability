<script lang="ts">
    import { Dot, Line, Plot } from "svelteplot";
    import {
        map_state_to_color,
        PlotDatumState,
        type PlotDatum,
        type TestData,
    } from "../types";

    let {
        plot_data,
        selected,
    }: {
        plot_data: PlotDatum[];
        selected: TestData;
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
            filtered = filtered.filter((x) => x.state !== PlotDatumState.Crash);
        }

        return filtered;
    });

    const DEFAULT_BOUNDS = {
        min_x: 1,
        min_y: 1,
        max_x: 10,
        max_y: 10,
    };

    let { min_x, min_y, max_x, max_y } = $derived.by(() => {
        if (
            filtered_plot_data.length === 0 ||
            !compare_run_x ||
            !compare_run_y
        ) {
            return DEFAULT_BOUNDS;
        }

        let bounds = {
            min_x: Number.MAX_VALUE,
            min_y: Number.MAX_VALUE,
            max_x: 0,
            max_y: 0,
        };

        for (const datum of filtered_plot_data) {
            if (datum.runs[compare_run_x].ms_taken < bounds.min_x) {
                bounds.min_x = datum.runs[compare_run_x].ms_taken;
            }
            if (datum.runs[compare_run_y].ms_taken < bounds.min_y) {
                bounds.min_y = datum.runs[compare_run_y].ms_taken;
            }

            if (datum.runs[compare_run_x].ms_taken > bounds.max_x) {
                bounds.max_x = datum.runs[compare_run_x].ms_taken;
            }
            if (datum.runs[compare_run_y].ms_taken > bounds.max_y) {
                bounds.max_y = datum.runs[compare_run_y].ms_taken;
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

    let max_min = $derived(Math.max(min_x, min_y));
    let min_max = $derived(Math.min(max_x, max_y));

    let line_data = $derived([
        { x: max_min, y: max_min },
        { x: min_max, y: min_max },
    ]);

    // Comparison summary for the two selected runs
    let {
        compare_total,
        compare_faster,
        compare_slower,
        compare_same,
        compare_avg_pct,
    } = $derived.by(() => {
        if (
            !compare_run_x ||
            !compare_run_y ||
            filtered_plot_data.length === 0
        ) {
            return {
                compare_total: 0,
                compare_faster: 0,
                compare_slower: 0,
                compare_same: 0,
                compare_avg_pct: 0,
            };
        }

        let total = 0;
        let faster = 0;
        let slower = 0;
        let same = 0;
        let pctSum = 0;

        for (const d of filtered_plot_data) {
            const x = d.runs[compare_run_x].ms_taken;
            const y = d.runs[compare_run_y].ms_taken;
            if (!isFinite(x) || !isFinite(y) || x <= 0) continue;

            total += 1;
            let max = Math.max(x, y);
            const pct = ((y - x) / max) * 100; // positive => Y slower, negative => Y faster
            pctSum += pct;

            if (Math.abs(pct) <= 5) {
                same += 1;
            } else if (pct < -5) {
                slower += 1;
            } else {
                faster += 1;
            }
        }

        return {
            compare_total: total,
            compare_faster: faster,
            compare_slower: slower,
            compare_same: same,
            compare_avg_pct: total > 0 ? pctSum / total : 0,
        };
    });
</script>

<div class="container flex-row">
    <span>Run on X axis:</span>
    {#each selected.tool_results as result}
        <button
            type="button"
            class:is-selected={compare_run_x === result.run_name}
            onclick={() => {
                compare_run_x = result.run_name;
            }}
        >
            {result.run_name}
        </button>
    {/each}
</div>
<div class="container flex-row">
    <span>Run on Y axis:</span>
    {#each selected.tool_results as result}
        <button
            type="button"
            class:is-selected={compare_run_y === result.run_name}
            onclick={() => {
                compare_run_y = result.run_name;
            }}
        >
            {result.run_name}
        </button>
    {/each}
</div>
<div class="container flex-row">
    <button
        type="button"
        class:is-selected={filter_crashes}
        onclick={() => {
            filter_crashes = !filter_crashes;
        }}
    >
        filter crashes
    </button>
    <button
        type="button"
        class:is-selected={sync_bounds}
        onclick={() => {
            sync_bounds = !sync_bounds;
        }}
    >
        sync bounds
    </button>
</div>
<div class="container flex-row">
    {#each Object.values(PlotDatumState) as state}
        <span
            ><div
                class="color-indicator"
                style="background-color: {map_state_to_color(state)};"
            ></div>
            {state}</span
        >
    {/each}
</div>
<div
    class="container plot-container"
    bind:clientWidth={plot_container_width}
    bind:clientHeight={plot_container_height}
>
    {#if compare_run_x && compare_run_y && plot_container_width && plot_container_height}
        <Plot
            axes
            grid
            x={{ type: "log", domain: [min_x, max_x] }}
            y={{ type: "log", domain: [min_y, max_y] }}
            width={plot_container_width}
            height={plot_container_height}
        >
            <Dot
                data={filtered_plot_data}
                x={(d) => d.runs[compare_run_x!].ms_taken}
                y={(d) => d.runs[compare_run_y!].ms_taken}
                stroke={(d) => map_state_to_color(d.state)}
            />
            <Line
                data={line_data}
                x="x"
                y="y"
                stroke="#ffffff80"
                strokeDasharray="10"
            ></Line>
        </Plot>
    {:else}
        <span> Please select tools to display above. </span>
    {/if}
</div>
<div class="container">
    {#if compare_run_x && compare_run_y}
        {#if compare_total > 0}
            <div
                class="compare-overview"
                style="font-size:0.95rem; line-height:1.3"
            >
                <div><strong>{compare_run_x}</strong></div>
                <div>
                    {compare_faster} faster • {compare_slower} slower • {compare_same}
                    same (±5%)
                </div>
                <div>
                    Average shift: {compare_avg_pct.toFixed(1)}%
                    <small style="color:#666"
                        >(negative = {compare_run_y} faster)</small
                    >
                </div>
            </div>
        {:else}
            <div>No comparable data</div>
        {/if}
    {:else}
        <div>Select two runs to compare</div>
    {/if}
</div>
