// Right-rail accordion container.
//
// Renders every section in display order. Each section is a thin wrapper
// around an existing panel component (PalettePanel → Color, LayerPanel →
// Layers, etc) plus the new BrushSection/FillSection that absorb the old
// ToolOptionsPanel.

import { type Component } from "solid-js";
import { activeSpriteId } from "../canvas/canvas-state";
import RailSection from "./RailSection";
import PalettePanel from "../palette/PalettePanel";
import LayerPanel from "../layers/LayerPanel";
import TilemapPanel from "../tilemap/TilemapPanel";
import SheetView from "../sheet/SheetView";
import BrushSection from "../canvas/tools/BrushSection";
import FillSection from "../canvas/tools/FillSection";
import SelectSection from "../canvas/select/SelectSection";
import DitheringSection from "../canvas/tools/DitheringSection";
import FxSection from "../canvas/tools/FxSection";

const RightRail: Component = () => {
  return (
    <aside class="right-rail" data-testid="right-rail">
      <RailSection id="brush">
        <BrushSection />
      </RailSection>
      <RailSection id="fill">
        <FillSection />
      </RailSection>
      <RailSection id="select">
        <SelectSection />
      </RailSection>
      <RailSection id="dithering">
        <DitheringSection />
      </RailSection>
      <RailSection id="color">
        <PalettePanel spriteId={activeSpriteId()} inRail />
      </RailSection>
      <RailSection id="layers">
        <LayerPanel inRail />
      </RailSection>
      <RailSection id="tilemap">
        <TilemapPanel inRail />
      </RailSection>
      <RailSection id="fx">
        <FxSection />
      </RailSection>
      <RailSection id="reference">
        <SheetView inRail />
      </RailSection>
    </aside>
  );
};

export default RightRail;
