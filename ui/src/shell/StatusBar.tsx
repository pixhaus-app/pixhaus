import type { Component } from "solid-js";
import { theme } from "../preferences/preferences-store";
import { projectState } from "../project-state";

const StatusBar: Component = () => {
  return (
    <div class="status-bar">
      <div class="status-bar__left">
        <span class="status-bar__item">
          {projectState.activeProject !== null
            ? (projectState.activeProject?.metadata.name ?? "Untitled")
            : "No project open"}
        </span>
      </div>
      <div class="status-bar__right">
        <span class="status-bar__item">{theme()}</span>
        <span class="status-bar__item">Pixhaus</span>
      </div>
    </div>
  );
};

export default StatusBar;
