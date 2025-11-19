import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'

function parse_cmd_argument(argument: string): string {
    let full_argument = process.argv.find(x => x.startsWith(`--${argument}=`));
    if (full_argument) {
        return full_argument.substring(3 + argument.length);
    }
    throw new Error(`Argument "--${argument}=..." is required but was now provided`);
}

function parse_as_number(value: unknown): number {
    let parsed = Number(value);
    if (Number.isNaN(parsed)) {
        throw new Error(`Failed to parse "${value}" as number`);
    }
    return parsed;
}

const server_port = parse_as_number(parse_cmd_argument("server_port"));
const ui_port = parse_as_number(parse_cmd_argument("ui_port"));


// https://vite.dev/config/
export default defineConfig({
    plugins: [svelte()],
    server: {
        port: ui_port
    },
    define: {
        SERVER_PORT: server_port
    }
})
