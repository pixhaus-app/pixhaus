---
title: Your first tilemap
description: Create a tileset and paint a small level using autotile rules.
---

import { Steps, Aside } from "@astrojs/starlight/components";

<Steps>
1. **Add a tilemap layer.** In the layer panel, click `+` and choose `Tilemap layer`. A dialog asks for tile dimensions — use `16x16`. The layer panel shows the new tilemap layer.

2. **Create a tileset.** The tileset panel opens alongside the canvas. Click `New tileset` and give it a name (e.g., `grass`). Start drawing tiles: each cell in the tileset grid is a 16x16 tile you can paint on.

3. **Draw your tiles.** Draw at least:
   - A solid ground tile
   - A top-edge tile (grass top)
   - Corner variants for rounded edges

4. **Configure autotile.** Right-click the tileset in the tileset panel and choose `Configure autotile`. Select `Wang edge-blob (47 tiles)`. Map your drawn tiles to the blob positions. Pixhaus highlights which positions need which transition variant.

5. **Paint the map.** Switch to the canvas, make sure the tilemap layer is active, and paint. With autotile enabled, adjacent tiles update automatically to show the correct edge and corner transitions.

6. **Preview at game scale.** Use the zoom controls to view at 100% (1:1 pixel). The tilemap should tile seamlessly.

7. **Export.** `File > Export > TMX tilemap` to export to Tiled format for Unity's SuperTiled2Unity importer.
</Steps>

<Aside>
The AI verb **Tile** can generate a full 47-tile blob autotile set from just 1–3 example transitions. See [Tile](/ai-verbs/tile/) for details.
</Aside>

## Next steps

- Read the full [tilemaps reference](/tilemaps/overview/)
- [Export to Unity](/animation/export/)
