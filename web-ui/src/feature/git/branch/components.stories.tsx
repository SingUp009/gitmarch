import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { vi } from "vitest";
import type { GitOperationResponse } from "../response";
import { BranchList } from "./components";

vi.mock("infrastructure/client", () => ({
  default: { get: vi.fn(), post: vi.fn() },
}));

import client from "infrastructure/client";

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

const meta = {
  component: BranchList,
  args: {
    repository: "test-repo",
  },
} satisfies Meta<typeof BranchList>;

export default meta;
type Story = StoryObj<typeof meta>;

export const WithBranches: Story = {
  name: "ブランチあり",
  beforeEach: () => {
    mockGet.mockResolvedValue(makeResponse("* main\n  develop\n  feature/new-ui\n"));
  },
};

export const SingleBranch: Story = {
  name: "ブランチ1本",
  beforeEach: () => {
    mockGet.mockResolvedValue(makeResponse("* main\n"));
  },
};

export const Empty: Story = {
  name: "ブランチなし",
  beforeEach: () => {
    mockGet.mockResolvedValue(makeResponse(""));
  },
};

export const LoadError: Story = {
  name: "読み込みエラー",
  beforeEach: () => {
    mockGet.mockResolvedValue(makeResponse("", false));
  },
};
