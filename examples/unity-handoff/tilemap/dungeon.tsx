<?xml version="1.0" encoding="UTF-8"?>
<!--
  Tileset for dungeon.tmx. Five real tiles packed in a single horizontal row.

  Pixhaus TileIndex -> atlas local id (= GID - firstgid):
    1  floor     (stone floor tile)        -> local id 0
    2  wall      (outer wall)              -> local id 1
    3  corner    (inner corner)            -> local id 2
    4  torch     (wall torch, left-facing) -> local id 3
    5  chest     (treasure chest)          -> local id 4

  TileIndex(0) is the project-side "empty" sentinel and is not in the
  atlas; empty cells encode as gid=0 directly in the TMX layer data.

  Atlas layout: dungeon.png is 80×16 pixels (5 tiles × 16px wide, 1 row).
  Column offset for atlas local id N = N * 16.
-->
<tileset version="1.10" tiledversion="1.10.0"
         name="dungeon"
         tilewidth="16" tileheight="16"
         spacing="0" margin="0"
         tilecount="5" columns="5">
  <image source="dungeon.png" width="80" height="16"/>
</tileset>
