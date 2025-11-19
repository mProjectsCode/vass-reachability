import type { TestData } from "./types";

declare global {
    interface Window {
        SERVER_PORT: number,
    }
}

function get_server_addr(): string {
    return `http://localhost:${window.SERVER_PORT}`
}

export async function API_list_test_folders(): Promise<string[]> {
    let res = await fetch(`${get_server_addr()}/api/list_test_folders`, {
        method: "GET"
    });
    let json = await res.json();
    return json as string[];
}

export async function API_test_data(test_folder: string): Promise<TestData> {
    let res = await fetch(`${get_server_addr()}/api/test_data`, {
        method: "POST",
        body: JSON.stringify(test_folder),
        headers: {
            "Content-Type": "application/json"
        }
    });
    let json = await res.json();
    return json as TestData;
}