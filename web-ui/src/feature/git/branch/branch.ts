import client from "infrastructure/client";
import { GitOperationResponse } from "../response";

export interface Branch {
    name: string;
}

export const getBranches = async (repoPath: string): Promise<Branch[]> => {
    const response = await client.get<GitOperationResponse>(`/git/branch?path=${encodeURIComponent(repoPath)}`);
    if (!response.success) {
        throw new Error(response.stderr || `git branch failed (exit_code=${response.exit_code ?? "unknown"})`);
    }

    return parseBranches(response.stdout);
}

function parseBranches(stdout: string): Branch[] {
    return stdout
        .split("\n")
        .map((line) => line.replace(/^\*\s*/, "").trim())
        .filter((name) => name.length > 0)
        .reverse()
        .map((name) => ({ name }));
}
