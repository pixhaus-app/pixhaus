<?xml version="1.0" encoding="UTF-8"?>
<!--
  Tileset for dungeon.tmx. Six tiles packed in a single horizontal row.

  Tile index assignments:
    0  empty     (fully transparent — reserved, never referenced directly)
    1  floor     (stone floor tile)
    2  wall      (outer wall)
    3  corner    (inner corner where wall meets floor)
    4  torch     (wall torch, left-facing; flip X for right-facing)
    5  chest     (treasure chest)

  Atlas layout: dungeon.png is 96×16 pixels (6 tiles × 16px wide, 1 row).
  Column offset for tile N = N * 16.
-->
<tileset version="1.10" tiledversion="1.10.0"
         name="dungeon"
         tilewidth="16" tileheight="16"
         spacing="0" margin="0"
         tilecount="6" columns="6">
  <image source="dungeon.png" width="96" height="16"/>
</tileset>
