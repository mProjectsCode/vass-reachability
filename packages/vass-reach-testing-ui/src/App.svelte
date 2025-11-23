<script lang="ts">
    import { API_list_test_folders, API_test_data } from "./fetch";
    import Plot from "./Plot.svelte";
    import { type PlotDatum, PlotDatumState, map_datum_state } from "./types";

    let selected: string | undefined = $state();
    let selected_data = $derived(selected ? await API_test_data(selected) : undefined);

    let plot_data = $derived.by(() => {
        if (!selected_data) {
            return [];
        }

        let map = new Map<string, PlotDatum>();

        for (const tool_result of selected_data.tool_results) {
            for (const [net, result] of Object.entries(tool_result.results)) {
                let existing = map.get(net) ?? {
                    state: undefined as unknown as PlotDatumState,
                    times: {},
                    net: net,
                };
                existing.state = map_datum_state(existing.state, result.result);
                existing.times[tool_result.tool_name] = result.ms_taken;
                map.set(net, existing);
            }
        }

        return [...map.values()];
    });



    let conflict_data = $derived(plot_data.filter(x => x.state === PlotDatumState.Conflict));
    
    let test_folders = await API_list_test_folders();
</script>

<main>
    <div class="container flex-row">
        {#each test_folders as folder}
            <button type="button" class:is-selected={selected === folder} onclick={() => { selected = folder }}>
                {folder}
            </button>
        {/each}
    </div>
    {#if selected_data}
        <Plot plot_data={plot_data} selected={selected_data}></Plot>

        <div class="container">
            {#if conflict_data.length > 0}
                Conflicts:
                <ul>
                    {#each conflict_data as datum}
                        <li>{datum.net}</li>
                    {/each}
                </ul>
            {/if}
        </div>
        <div class="container">
            <pre><code>{JSON.stringify(selected_data, null, 4)}</code></pre>
        </div>
    {/if}
</main>
