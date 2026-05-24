import { describe, expect, it, vi } from "vitest";
import { registerEventRouter, type EventPayload, type ListenFn } from "./events";

/** A fake listen that records handlers so a test can fire events at them. */
function fakeListen(): {
  listen: ListenFn;
  fire: (event: string, payload: unknown) => void;
  unlistenFor: (event: string) => ReturnType<typeof vi.fn>;
} {
  const handlers = new Map<string, (e: EventPayload<unknown>) => void>();
  const unlistens = new Map<string, ReturnType<typeof vi.fn>>();
  const listen: ListenFn = <T>(event: string, handler: (e: EventPayload<T>) => void) => {
    handlers.set(event, handler as (e: EventPayload<unknown>) => void);
    const off = vi.fn();
    unlistens.set(event, off);
    return Promise.resolve(off);
  };
  return {
    listen,
    fire: (event, payload) => handlers.get(event)?.({ event, payload }),
    unlistenFor: (event) => unlistens.get(event)!,
  };
}

const tick = () => new Promise((r) => setTimeout(r, 0));

describe("registerEventRouter", () => {
  it("routes an event payload to the matching handler", async () => {
    const { listen, fire } = fakeListen();
    const handle = vi.fn();
    registerEventRouter(listen, [{ event: "canvas:tile-dirty", handle }]);
    await tick();
    fire("canvas:tile-dirty", { x: 1 });
    expect(handle).toHaveBeenCalledWith({ x: 1 });
  });

  it("routes each event to its own handler", async () => {
    const { listen, fire } = fakeListen();
    const menu = vi.fn();
    const update = vi.fn();
    registerEventRouter(listen, [
      { event: "shell:menu", handle: menu },
      { event: "updater:available", handle: update },
    ]);
    await tick();
    fire("shell:menu", "save");
    expect(menu).toHaveBeenCalledWith("save");
    expect(update).not.toHaveBeenCalled();
  });

  it("detaches all subscriptions on cleanup", async () => {
    const { listen, unlistenFor } = fakeListen();
    const dispose = registerEventRouter(listen, [{ event: "shell:menu", handle: vi.fn() }]);
    await tick();
    dispose();
    expect(unlistenFor("shell:menu")).toHaveBeenCalledTimes(1);
  });

  it("detaches subscriptions that resolve after an early dispose", async () => {
    const { listen, unlistenFor } = fakeListen();
    const dispose = registerEventRouter(listen, [{ event: "shell:menu", handle: vi.fn() }]);
    // Dispose before the listen promise resolves.
    dispose();
    await tick();
    expect(unlistenFor("shell:menu")).toHaveBeenCalledTimes(1);
  });
});
