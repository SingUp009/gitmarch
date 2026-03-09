import { describe, it, expect } from "vitest";
import { cn } from "./utils";

describe("cn", () => {
  it("単一クラスをそのまま返す", () => {
    expect(cn("foo")).toBe("foo");
  });

  it("複数クラスをスペース区切りで結合する", () => {
    expect(cn("foo", "bar")).toBe("foo bar");
  });

  it("falsy な値を除外する", () => {
    expect(cn("foo", false, undefined, null, "bar")).toBe("foo bar");
  });

  it("条件付きクラスオブジェクトを展開する", () => {
    expect(cn({ active: true, disabled: false })).toBe("active");
  });

  it("Tailwind の競合クラスを後勝ちでマージする", () => {
    // p-2 と p-4 が競合 → 後者 p-4 が勝つ
    expect(cn("p-2", "p-4")).toBe("p-4");
  });

  it("Tailwind の異なる方向プロパティは競合しない", () => {
    // px と py は独立
    expect(cn("px-2", "py-4")).toBe("px-2 py-4");
  });

  it("引数なしで空文字を返す", () => {
    expect(cn()).toBe("");
  });

  it("配列形式の入力を扱える", () => {
    expect(cn(["foo", "bar"])).toBe("foo bar");
  });
});
