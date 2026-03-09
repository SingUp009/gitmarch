import { describe, it, expect, vi, afterEach } from "vitest";
import { render, cleanup } from "@testing-library/react";

// usePathname だけモック（next/link は process.env の define で実物を使用）
vi.mock("next/navigation", () => ({
  usePathname: vi.fn(),
}));

import { usePathname } from "next/navigation";
import { AppSidebar } from "./AppSidebar";

const mockPathname = vi.mocked(usePathname);

afterEach(cleanup);

describe("AppSidebar", () => {
  it("ロゴ『gitmarch』が表示される", () => {
    mockPathname.mockReturnValue("/");
    const { getByText } = render(<AppSidebar />);
    expect(getByText("gitmarch")).toBeTruthy();
  });

  it("4つのナビ項目が表示される", () => {
    mockPathname.mockReturnValue("/");
    const { getByText } = render(<AppSidebar />);
    expect(getByText("ダッシュボード")).toBeTruthy();
    expect(getByText("リポジトリ")).toBeTruthy();
    expect(getByText("ユーザー")).toBeTruthy();
    expect(getByText("設定")).toBeTruthy();
  });

  it("/ のとき『ダッシュボード』がアクティブクラスを持つ", () => {
    mockPathname.mockReturnValue("/");
    const { getByText } = render(<AppSidebar />);
    const link = getByText("ダッシュボード").closest("a");
    expect(link?.className).toContain("text-sidebar-primary");
  });

  it("/repos のとき『リポジトリ』がアクティブクラスを持つ", () => {
    mockPathname.mockReturnValue("/repos");
    const { getByText } = render(<AppSidebar />);
    const link = getByText("リポジトリ").closest("a");
    expect(link?.className).toContain("text-sidebar-primary");
  });

  it("/repos のとき『ダッシュボード』はアクティブでない", () => {
    mockPathname.mockReturnValue("/repos");
    const { getByText } = render(<AppSidebar />);
    const link = getByText("ダッシュボード").closest("a");
    expect(link?.className).not.toContain("text-sidebar-primary");
  });

  it("バージョン文字列が表示される", () => {
    mockPathname.mockReturnValue("/");
    const { getByText } = render(<AppSidebar />);
    expect(getByText("gitmarch v0.1.0")).toBeTruthy();
  });
});
