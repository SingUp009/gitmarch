import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, waitFor, fireEvent, cleanup } from "@testing-library/react";
import type { GitOperationResponse } from "../response";

vi.mock("infrastructure/client", () => ({
  default: { get: vi.fn(), post: vi.fn() },
}));

import client from "infrastructure/client";
import { BranchList } from "./components";

const mockGet = vi.mocked(client.get);

function makeResponse(stdout: string, success = true): GitOperationResponse {
  return {
    success,
    exit_code: success ? 0 : 1,
    stdout,
    stderr: success ? "" : "fatal: not a git repository",
    cwd: "/repos/test",
    command: ["git", "branch"],
  };
}

beforeEach(() => {
  mockGet.mockReset();
});

afterEach(cleanup);

describe("BranchList", () => {
  it("フェッチ後に SelectTrigger がレンダリングされる", async () => {
    mockGet.mockResolvedValue(makeResponse("* main\n  develop\n"));
    const { getByRole } = render(<BranchList repository="test-repo" />);

    // Select の combobox が存在することを確認（閉じた状態でも見える）
    await waitFor(() => {
      expect(getByRole("combobox")).toBeTruthy();
    });
  });

  it("Select を開くとブランチ一覧が表示される", async () => {
    mockGet.mockResolvedValue(makeResponse("* main\n  develop\n  feature/foo\n"));
    const { getByRole, getAllByRole } = render(<BranchList repository="test-repo" />);

    // フェッチ完了を待つ
    await waitFor(() => {
      expect(mockGet).toHaveBeenCalledOnce();
    });

    // Select を開く
    fireEvent.click(getByRole("combobox"));

    // オプションが3つ表示される
    await waitFor(() => {
      const options = getAllByRole("option");
      expect(options).toHaveLength(3);
    });
  });

  it("ブランチが空のとき Select を開いてもオプションが表示されない", async () => {
    mockGet.mockResolvedValue(makeResponse(""));
    const { getByRole, queryAllByRole } = render(<BranchList repository="test-repo" />);

    await waitFor(() => {
      expect(mockGet).toHaveBeenCalledOnce();
    });

    fireEvent.click(getByRole("combobox"));

    await waitFor(() => {
      expect(queryAllByRole("option")).toHaveLength(0);
    });
  });

  it("APIエラー時にブランチが空のまま Select が表示される", async () => {
    mockGet.mockResolvedValue(makeResponse("", false));
    const { getByRole, queryAllByRole } = render(<BranchList repository="test-repo" />);

    await waitFor(() => {
      expect(mockGet).toHaveBeenCalledOnce();
    });

    fireEvent.click(getByRole("combobox"));

    await waitFor(() => {
      expect(queryAllByRole("option")).toHaveLength(0);
    });
  });

  it("repository prop が変わると再フェッチする", async () => {
    mockGet.mockResolvedValue(makeResponse("* main\n"));
    const { rerender } = render(<BranchList repository="repo-a" />);

    await waitFor(() => {
      expect(mockGet).toHaveBeenCalledWith(
        expect.stringContaining(encodeURIComponent("repo-a")),
      );
    });

    mockGet.mockResolvedValue(makeResponse("* develop\n"));
    rerender(<BranchList repository="repo-b" />);

    await waitFor(() => {
      expect(mockGet).toHaveBeenCalledWith(
        expect.stringContaining(encodeURIComponent("repo-b")),
      );
    });
  });
});
