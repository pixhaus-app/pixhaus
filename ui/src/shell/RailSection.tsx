// Accordion section primitive for the right rail.
//
// Renders a clickable header (title + chevron) and, when open, the body.
// Each section owns its open/closed state via rail-state. Clicking the
// header toggles userTouched so auto-expand rules stop driving it.

import { Show, type Component, type JSX } from "solid-js";
import { HelpCircle } from "lucide-solid";
import {
  isSectionOpen,
  toggleSection,
  SECTION_TITLE,
  SECTION_HELP,
  type SectionId,
} from "./rail-state";
import Tooltip from "../lib/ui/Tooltip";

type Props = {
  id: SectionId;
  /** Optional override of the rail-state title (e.g. for context-dependent labels). */
  title?: string;
  /** Optional inline actions rendered to the right of the title (e.g. add button). */
  actions?: JSX.Element;
  children: JSX.Element;
};

const RailSection: Component<Props> = (props) => {
  const open = () => isSectionOpen(props.id);
  return (
    <section class="rail-section" data-section={props.id} data-open={open()}>
      <div class="rail-section__head">
        <button
          type="button"
          class="rail-section__header"
          aria-expanded={open()}
          aria-controls={`rail-section-${props.id}`}
          data-testid={`rail-section-toggle-${props.id}`}
          onClick={() => toggleSection(props.id)}
        >
          <svg
            class="rail-section__chevron"
            width="8"
            height="8"
            viewBox="0 0 8 8"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d={open() ? "M1 2 L4 6 L7 2" : "M2 1 L6 4 L2 7"} />
          </svg>
          <span class="rail-section__title">{props.title ?? SECTION_TITLE[props.id]}</span>
        </button>
        <Show when={props.actions}>
          <span class="rail-section__actions" data-testid={`rail-section-actions-${props.id}`}>
            {props.actions}
          </span>
        </Show>
        <Tooltip
          label={props.title ?? SECTION_TITLE[props.id]}
          description={SECTION_HELP[props.id]}
          placement="left"
        >
          <button
            type="button"
            class="rail-section__help"
            aria-label={`About ${props.title ?? SECTION_TITLE[props.id]}`}
            data-testid={`rail-section-help-${props.id}`}
          >
            <HelpCircle size={13} aria-hidden={true} />
          </button>
        </Tooltip>
      </div>
      <Show when={open()}>
        <div class="rail-section__body" id={`rail-section-${props.id}`}>
          {props.children}
        </div>
      </Show>
    </section>
  );
};

export default RailSection;
