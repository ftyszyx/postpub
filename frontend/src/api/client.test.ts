import { describe, expect, it } from "vitest";
import { buildUrl, encodePathSegments } from "./client";

describe("api client helpers", () => {
  it("encodes path segments safely", () => {
    expect(encodePathSegments("folder name/article 1.html")).toBe(
      "folder%20name/article%201.html"
    );
  });

  it("builds query strings for relative api paths", () => {
    expect(buildUrl("/api/templates", { category: "general", page: 2 })).toBe(
      "/api/templates?category=general&page=2"
    );
  });
});
