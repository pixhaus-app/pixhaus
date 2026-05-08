// Typed accessors for window.__pixhaus_debug__.
//
// Every helper here is a thin wrapper around browser.execute() that
// returns a snapshot of UI state. The functions mirror the surface
// declared in ui/src/lib/debug/index.ts — when you add a new accessor
// there, mirror it here.

import { browser } from "@wdio/globals";

/**
 * Asserts the debug surface was installed by main.tsx and returns when
 * it's ready. Call once at the top of every spec's `before` hook.
 */
export async function waitForDebugSurface(timeoutMs = 10000): Promise<void> {
  await browser.waitUntil(
    async () => {
      const present = await browser.execute(() => {
        return (
          typeof (window as unknown as { __pixhaus_debug__?: unknown })
            .__pixhaus_debug__ !== "undefined"
        );
      });
      return present === true;
    },
    {
      timeout: timeoutMs,
      timeoutMsg:
        "window.__pixhaus_debug__ never appeared (was main.tsx loaded?)",
    },
  );
}

export async function getActiveProject(): Promise<unknown> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { getActiveProject(): unknown };
    };
    return w.__pixhaus_debug__.getActiveProject();
  });
}

export async function getActiveSpriteId(): Promise<number | null> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { getActiveSpriteId(): number | null };
    };
    return w.__pixhaus_debug__.getActiveSpriteId();
  });
}

export async function getActiveLayerId(): Promise<number | null> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { getActiveLayerId(): number | null };
    };
    return w.__pixhaus_debug__.getActiveLayerId();
  });
}

export async function getActiveFrameIndex(): Promise<number> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { getActiveFrameIndex(): number };
    };
    return w.__pixhaus_debug__.getActiveFrameIndex();
  });
}

export async function getZoom(): Promise<number> {
  return browser.execute(() => {
    const w = window as unknown as { __pixhaus_debug__: { getZoom(): number } };
    return w.__pixhaus_debug__.getZoom();
  });
}

export async function getScroll(): Promise<{ x: number; y: number }> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { getScroll(): { x: number; y: number } };
    };
    return w.__pixhaus_debug__.getScroll();
  });
}

export async function getCrashReportingDialogShown(): Promise<boolean> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { getCrashReportingDialogShown(): boolean };
    };
    return w.__pixhaus_debug__.getCrashReportingDialogShown();
  });
}

export async function getCrashReportingEnabled(): Promise<boolean> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { getCrashReportingEnabled(): boolean };
    };
    return w.__pixhaus_debug__.getCrashReportingEnabled();
  });
}

export async function getLayerCount(): Promise<number> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { getLayerCount(): number };
    };
    return w.__pixhaus_debug__.getLayerCount();
  });
}

export async function getFrameCount(): Promise<number> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { getFrameCount(): number };
    };
    return w.__pixhaus_debug__.getFrameCount();
  });
}

export async function isCommandPaletteOpen(): Promise<boolean> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: { isCommandPaletteOpen(): boolean };
    };
    return w.__pixhaus_debug__.isCommandPaletteOpen();
  });
}

export async function getPanelVisibility(): Promise<{
  layers: boolean;
  timeline: boolean;
  palette: boolean;
  tilemap: boolean;
}> {
  return browser.execute(() => {
    const w = window as unknown as {
      __pixhaus_debug__: {
        panel: {
          layers(): boolean;
          timeline(): boolean;
          palette(): boolean;
          tilemap(): boolean;
        };
      };
    };
    return {
      layers: w.__pixhaus_debug__.panel.layers(),
      timeline: w.__pixhaus_debug__.panel.timeline(),
      palette: w.__pixhaus_debug__.panel.palette(),
      tilemap: w.__pixhaus_debug__.panel.tilemap(),
    };
  });
}
