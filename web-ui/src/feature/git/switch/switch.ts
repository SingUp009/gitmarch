import client from "infrastructure/client";
import { GitOperationResponse } from "../response";

const assertSwitchSucceeded = (response: GitOperationResponse): GitOperationResponse => {
    if (!response.success) {
        throw new Error(response.stderr || `git switch failed (exit_code=${response.exit_code ?? "unknown"})`);
    }
    return response;
};

export const switchBranch = async (repoPath: string, branchName: string): Promise<GitOperationResponse> => {
    const path = encodeURIComponent(repoPath);
    const branch = encodeURIComponent(branchName);

    const response = await client.get<GitOperationResponse>(`/git/switch?path=${path}&arg%5B%5D=${branch}`);
    return assertSwitchSucceeded(response);
};
