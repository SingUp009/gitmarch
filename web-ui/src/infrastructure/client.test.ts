import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// fetch をモックしてから client をインポート
const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

// 環境変数をリセットしてからインポートするため動的 import を使用
let client: typeof import("./client").default;

beforeEach(async () => {
  vi.resetModules();
  mockFetch.mockReset();
  client = (await import("./client")).default;
});

afterEach(() => {
  vi.unstubAllEnvs();
});

function makeResponse(ok: boolean, status: number, body: unknown) {
  return {
    ok,
    status,
    json: () => Promise.resolve(body),
  };
}

describe("client.get", () => {
  it("成功レスポンスを JSON としてパースして返す", async () => {
    const payload = { success: true, stdout: "main\n" };
    mockFetch.mockResolvedValueOnce(makeResponse(true, 200, payload));

    const result = await client.get("/git/branch?path=repo");

    expect(result).toEqual(payload);
    expect(mockFetch).toHaveBeenCalledOnce();
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/git/branch?path=repo"),
    );
  });

  it("デフォルトベース URL (http://127.0.0.1:8080) を使用する", async () => {
    mockFetch.mockResolvedValueOnce(makeResponse(true, 200, {}));

    await client.get("/test");

    expect(mockFetch).toHaveBeenCalledWith("http://127.0.0.1:8080/test");
  });

  it("HTTP エラー (404) で例外を投げる", async () => {
    mockFetch.mockResolvedValueOnce(makeResponse(false, 404, {}));

    await expect(client.get("/not-found")).rejects.toThrow("HTTP error! status: 404");
  });

  it("HTTP エラー (500) で例外を投げる", async () => {
    mockFetch.mockResolvedValueOnce(makeResponse(false, 500, {}));

    await expect(client.get("/error")).rejects.toThrow("HTTP error! status: 500");
  });

  it("ネットワークエラーで例外を伝播する", async () => {
    mockFetch.mockRejectedValueOnce(new Error("Network failure"));

    await expect(client.get("/any")).rejects.toThrow("Network failure");
  });
});

describe("client.post", () => {
  it("成功レスポンスを JSON としてパースして返す", async () => {
    const payload = { id: 1 };
    mockFetch.mockResolvedValueOnce(makeResponse(true, 200, payload));

    const result = await client.post("/users", { name: "alice" });

    expect(result).toEqual(payload);
  });

  it("Content-Type: application/json ヘッダーを付与する", async () => {
    mockFetch.mockResolvedValueOnce(makeResponse(true, 200, {}));

    await client.post("/users", { name: "alice" });

    expect(mockFetch).toHaveBeenCalledWith(
      expect.any(String),
      expect.objectContaining({
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name: "alice" }),
      }),
    );
  });

  it("HTTP エラー (422) で例外を投げる", async () => {
    mockFetch.mockResolvedValueOnce(makeResponse(false, 422, {}));

    await expect(client.post("/users", {})).rejects.toThrow("HTTP error! status: 422");
  });
});
