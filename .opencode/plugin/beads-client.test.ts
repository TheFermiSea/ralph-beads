import { expect, test, mock, describe } from "bun:test";
import { BeadsClient } from "./beads-client";

describe("BeadsClient", () => {
  const createMockShell = (responseObj: any) => {
    const fn = (_strings: any, ..._values: any[]) => {
      return {
        quiet: () => Promise.resolve({
          text: () => JSON.stringify(responseObj),
          stdout: Buffer.from(JSON.stringify(responseObj)),
          exitCode: 0
        }),
        nothrow: () => ({
          quiet: () => Promise.resolve({ exitCode: 0 })
        }),
        stdin: () => ({
            quiet: () => Promise.resolve({ exitCode: 0 })
        })
      };
    };
    return fn;
  };

  test("info parses JSON output", async () => {
    const client = new BeadsClient(createMockShell({ version: "1.0.0" }));
    const result = await client.info();
    expect(result.version).toBe("1.0.0");
  });

  test("create returns issue object", async () => {
    const issue = { id: "bd-123", title: "Test Issue" };
    const client = new BeadsClient(createMockShell(issue));
    const result = await client.create({ title: "Test Issue" });
    expect(result.id).toBe("bd-123");
  });

  test("list returns array", async () => {
    const issues = [{ id: "bd-1" }, { id: "bd-2" }];
    const client = new BeadsClient(createMockShell(issues));
    const result = await client.list();
    expect(result).toHaveLength(2);
    expect(result[0].id).toBe("bd-1");
  });

  test("handle error gracefully", async () => {
    const mockFail = (_strings: any) => ({
      quiet: () => Promise.resolve({
        text: () => "invalid json", // Invalid JSON
        stdout: Buffer.from("invalid json")
      })
    });
    const client = new BeadsClient(mockFail);
    const result = await client.info();
    expect(result).toEqual({}); // Should return empty object on parse error
  });
});
