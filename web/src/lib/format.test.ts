import { describe, expect, it } from "vitest";
import { shortDate, time, daysUntil, fileSize, relativeTime } from "./format";

describe("french formatting", () => {
  it("writes dates the french way", () => {
    expect(shortDate("2026-06-12T20:00:00")).toBe("12/06/2026");
    expect(time("2026-06-12T20:05:00")).toBe("20h05");
  });

  it("says what is pressing in plain words", () => {
    const inThreeDays = new Date(Date.now() + 3 * 86_400_000).toISOString();
    expect(relativeTime(inThreeDays)).toBe("dans 3 jours");
    expect(daysUntil(inThreeDays)).toBe(3);

    const yesterday = new Date(Date.now() - 26 * 3_600_000).toISOString();
    expect(relativeTime(yesterday)).toBe("il y a 1 jour");
  });

  it("gives a readable file size", () => {
    expect(fileSize(512)).toBe("512 o");
    expect(fileSize(2048)).toBe("2 ko");
    expect(fileSize(5 * 1024 * 1024)).toBe("5.0 Mo");
  });
});
