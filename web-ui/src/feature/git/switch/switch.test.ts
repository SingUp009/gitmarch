import { describe, it, expect, vi, beforeEach } from "vitest";
import type { GitOperationResponse } from "../response";

vi.mock("infrastructure/client", () => ({
  default: { get: vi.fn() },
}));

import client from "infrastructure/client";
import { switchBranch } from "./switch";

const mockGet = vi.mocked(client.get);

function makeResponse(overrides: Partial<GitOperationResponse> = {}): GitOperationResponse {
  return {
    success: true,
    exit_code: 0,
    stdout: "Switched to branch 'develop'",
    stderr: "",
    cwd: "/repos/test",
    command: ["git", "switch", "develop"],
    ...overrides,
  };
}

beforeEach(() => {
  mockGet.mockReset();
});

describe("switchBranch", () => {
  it("成功時にレスポンスをそのまま返す", async () => {
    const resp = makeResponse();
    mockGet.mockResolvedValueOnce(resp);

    const result = await switchBranch("my-repo", "develop");

    expect(result).toEqual(resp);
  });

  it("repoPath と branchName を URL エンコードして GET する", async () => {
    mockGet.mockResolvedValueOnce(makeResponse());

    await switchBranch("my repo/proj", "feature/new feature");

    const expectedPath = encodeURIComponent("my repo/proj");
    const expectedBranch = encodeURIComponent("feature/new feature");
    expect(mockGet).toHaveBeenCalledWith(
      `/git/switch?path=${expectedPath}&arg%5B%5D=${expectedBranch}`,
    );
  });

  it("success=false のとき stderr のメッセージで例外を投げる", async () => {
    mockGet.mockResolvedValueOnce(
      makeResponse({ success: false, stderr: "error: pathspec 'missing' did not match", exit_code: 1 }),
    );

    await expect(switchBranch("my-repo", "missing")).rejects.toThrow(
      "error: pathspec 'missing' did not match",
    );
  });

  it("success=false かつ stderr が空のとき exit_code を含むエラーメッセージで例外を投げる", async () => {
    mockGet.mockResolvedValueOnce(
      makeResponse({ success: false, stderr: "", exit_code: 128 }),
    );

    await expect(switchBranch("my-repo", "branch")).rejects.toThrow(
      "git switch failed (exit_code=128)",
    );
  });

  it("success=false かつ exit_code が null のとき unknown を含むエラーメッセージで例外を投げる", async () => {
    mockGet.mockResolvedValueOnce(
      makeResponse({ success: false, stderr: "", exit_code: null }),
    );

    await expect(switchBranch("my-repo", "branch")).rejects.toThrow(
      "git switch failed (exit_code=unknown)",
    );
  });
});
