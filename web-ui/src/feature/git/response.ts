export interface GitOperationResponse {
    success: boolean;
    exit_code: number | null;
    stdout: string;
    stderr: string;
    cwd: string;
    command: string[];
}