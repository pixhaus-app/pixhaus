// Application metadata commands.

import { invoke } from "../ipc";

export type AppAbout = {
  name: string;
  version: string;
};

/** Returns the application name and version for the About dialog. */
export function appAbout(): Promise<AppAbout> {
  return invoke<AppAbout>("app_about");
}
