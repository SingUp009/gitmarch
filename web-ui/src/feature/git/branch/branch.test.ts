import { describe, it, expect, vi, beforeEach } from "vitest";
import type { GitOperationResponse } from "../response";

// client をモック
vi.mock("infrastructure/client", () => ({
  default: { get: vi.fn() },
}));

import client from "infrastructure/client";
import { getBranches } from "./branch";

const mockGet = vi.mocked(client.get);

function makeResponse(overrides: Partial<GitOperationResponse> = {}): GitOperationResponse {
  return {
    success: true,
    exit_code: 0,
    stdout: "",
    stderr: "",
    cwd: "/repos/test",
    command: ["git", "branch"],
    ...overrides,
  };
}

beforeEach(() => {
  mockGet.mockReset();
});

describe("getBranches", () => {
  it("stdout のブランチ一覧を逆順で返す", async () => {
    mockGet.mockResolvedValueOnce(makeResponse({ stdout: "  main\n  develop\n  feature/foo\n" }));

    const branches = await getBranches("my-repo");

    // reverse() されるので feature/foo, develop, main の順
    expect(branches).toEqual([
      { name: "feature/foo" },
      { name: "develop" },
      { name: "main" },
    ]);
  });

  it("現在のブランチを示す * プレフィックスを除去する", async () => {
    mockGet.mockResolvedValueOnce(makeResponse({ stdout: "* main\n  develop\n" }));

    const branches = await getBranches("my-repo");

    expect(branches.map((b) => b.name)).not.toContain("* main");
    expect(branches.some((b) => b.name === "main")).toBe(true);
  });

  it("空行を除外する", async () => {
    mockGet.mockResolvedValueOnce(makeResponse({ stdout: "  main\n\n  develop\n" }));

    const branches = await getBranches("my-repo");

    expect(branches).toHaveLength(2);
  });

  it("stdout が空のとき空配列を返す", async () => {
    mockGet.mockResolvedValueOnce(makeResponse({ stdout: "" }));

    const branches = await getBranches("my-repo");

    expect(branches).toEqual([]);
  });

  it("repoPath を URL エンコードして GET する", async () => {
    mockGet.mockResolvedValueOnce(makeResponse({ stdout: "main\n" }));

    await getBranches("my repo/test");

    expect(mockGet).toHaveBeenCalledWith(
      `/git/branch?path=${encodeURIComponent("my repo/test")}`,
    );
  });

  it("success=false のとき stderr のメッセージで例外を投げる", async () => {
    mockGet.mockResolvedValueOnce(
      makeResponse({ success: false, stderr: "not a git repository", exit_code: 128 }),
    );

    await expect(getBranches("bad-repo")).rejects.toThrow("not a git repository");
  });

  it("success=false かつ stderr が空のとき exit_code を含むエラーメッセージで例外を投げる", async () => {
    mockGet.mockResolvedValueOnce(
      makeResponse({ success: false, stderr: "", exit_code: 1 }),
    );

    await expect(getBranches("bad-repo")).rejects.toThrow("git branch failed (exit_code=1)");
  });
});
