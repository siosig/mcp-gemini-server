import { describe, expect, it, vi } from "vitest";
import { withAbortableTimeout, withTimeout } from "../../src/utils/telemetry.js";

describe("withAbortableTimeout", () => {
  it("returns the result on normal completion and clears the timer", async () => {
    const result = await withAbortableTimeout(async () => "ok", 1000);
    expect(result).toBe("ok");
  });

  it("aborts the AbortSignal on timeout and propagates the error", async () => {
    const promise = withAbortableTimeout(async (signal) => {
      // Promise that waits for signal.aborted
      return new Promise<string>((_, reject) => {
        signal.addEventListener("abort", () => reject(signal.reason as Error), { once: true });
      });
    }, 50);

    await expect(promise).rejects.toThrow(/Request timed out after/);
  });

  it("passes an AbortSignal to the caller's fn", async () => {
    let receivedSignal: AbortSignal | undefined;
    await withAbortableTimeout(async (signal) => {
      receivedSignal = signal;
      return "done";
    }, 1000);
    expect(receivedSignal).toBeInstanceOf(AbortSignal);
    expect(receivedSignal?.aborted).toBe(false);
  });

  it("releases the timer even when fn throws (leak prevention)", async () => {
    const clearSpy = vi.spyOn(globalThis, "clearTimeout");
    await expect(
      withAbortableTimeout(async () => {
        throw new Error("boom");
      }, 1000),
    ).rejects.toThrow("boom");
    expect(clearSpy).toHaveBeenCalled();
    clearSpy.mockRestore();
  });
});

describe("withTimeout (for calls without AbortSignal support, e.g. Files API)", () => {
  it("returns the result on normal completion", async () => {
    const result = await withTimeout(Promise.resolve(42), 1000);
    expect(result).toBe(42);
  });

  it("rejects with an error on timeout", async () => {
    const slow = new Promise<number>((resolve) => setTimeout(() => resolve(1), 200));
    await expect(withTimeout(slow, 50)).rejects.toThrow(/Request timed out after/);
  });
});
