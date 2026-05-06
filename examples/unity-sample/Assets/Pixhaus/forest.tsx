<?xml version="1.0" encoding="UTF-8"?>
<!--
  Forest tileset for forest.tmx. Seventeen tiles packed in a single horizontal row.

  Atlas local id (= GID - firstgid) -> tile:
     0  grass          (bright green floor)
     1  dirt           (brown floor)
     2  stone floor    (light grey floor)
     3  water          (blue, animated — frames 3/4/5 cycle in 200 ms each)
     4  water-2        (animation frame 2)
     5  water-3        (animation frame 3)
     6  tree top       (dark green canopy)
     7  tree trunk     (dark brown, beneath tree top)
     8  tall grass     (medium green tuft)
     9  rock           (medium grey boulder)
    10  flowers        (white/yellow dots)
    11  fence-h        (horizontal fence rail)
    12  fence-v        (vertical fence post)
    13  chest-closed   (gold and brown)
    14  chest-open     (gold and brown, open lid)
    15  wall stone     (dark grey dressed stone)
    16  path stone     (medium stone cobble)

  TileIndex(0) is the project-side "empty" sentinel; encode empty cells as
  gid=0 in the TMX layer data.

  Atlas layout: tileset.png is 272x16 pixels (17 tiles x 16px wide, 1 row).
  Column offset for atlas local id N = N * 16.

  Note: the current TmxImporter produces static Tile assets. The animation
  metadata on tile ids 3-5 is preserved in the TSX for future importer
  support and for use with Tiled itself; Unity will show tile 3 (water-1)
  as a static tile until the importer is updated to emit AnimatedTile assets.
-->
<tileset version="1.10" tiledversion="1.10.0"
         name="forest"
         tilewidth="16" tileheight="16"
         spacing="0" margin="0"
         tilecount="17" columns="17">
  <image source="tileset.png" width="272" height="16"/>
  <tile id="3">
    <animation>
      <frame tileid="3" duration="200"/>
      <frame tileid="4" duration="200"/>
      <frame tileid="5" duration="200"/>
    </animation>
  </tile>
</tileset>
