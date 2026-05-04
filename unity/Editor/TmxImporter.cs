using System;
using System.Collections.Generic;
using System.IO;
using System.Xml.Linq;
using UnityEditor;
using UnityEditor.AssetImporters;
using UnityEngine;
using UnityEngine.Tilemaps;

namespace Pixhaus.Editor
{
    // ScriptedImporter for .tmx tilemap files exported by Pixhaus (docs/unity-handoff.md).
    //
    // Produces per-import:
    //   - Grid GameObject prefab (main asset)
    //   - Tilemap child per layer
    //   - Tile assets per unique tile across all tilesets (sub-assets)
    //
    // Co-located TSX and tileset PNG files are declared as source dependencies so
    // re-exporting from Pixhaus triggers a reimport automatically.

    [ScriptedImporter(1, "tmx")]
    public class TmxImporter : ScriptedImporter
    {
        [SerializeField] public float pixelsPerUnit = 16f;
        [SerializeField] public FilterMode filterMode = FilterMode.Point;

        public override void OnImportAsset(AssetImportContext ctx)
        {
            var tmx = XDocument.Load(ctx.assetPath);
            var map = tmx.Root;
            if (map == null)
            {
                ctx.LogImportError("TMX file has no root element.");
                return;
            }

            var tileWidth = (int)map.Attribute("tilewidth");
            var tileHeight = (int)map.Attribute("tileheight");
            var mapWidth = (int)map.Attribute("width");
            var mapHeight = (int)map.Attribute("height");

            var dir = Path.GetDirectoryName(ctx.assetPath);

            // Load all tilesets referenced in the map.
            var tilesets = LoadTilesets(ctx, map, dir, tileWidth, tileHeight);
            if (tilesets == null)
                return;

            // Build Tile assets indexed by global tile ID.
            var tileAssets = BuildTileAssets(ctx, tilesets);

            // Create the Grid prefab.
            var gridGO = new GameObject(Path.GetFileNameWithoutExtension(ctx.assetPath));
            var grid = gridGO.AddComponent<Grid>();
            grid.cellSize = new Vector3(
                (float)tileWidth / pixelsPerUnit,
                (float)tileHeight / pixelsPerUnit,
                0f
            );

            int layerIndex = 0;
            foreach (var layerElem in map.Elements("layer"))
            {
                ImportLayer(ctx, layerElem, gridGO, mapWidth, mapHeight, tileAssets, layerIndex);
                layerIndex++;
            }

            ctx.AddObjectToAsset("grid", gridGO);
            ctx.SetMainObject(gridGO);
        }

        // -------------------------------------------------------------------------

        private struct TilesetInfo
        {
            public int firstGid;
            public int tileCount;
            public int tileWidth;
            public int tileHeight;
            public Texture2D texture;
        }

        private List<TilesetInfo> LoadTilesets(
            AssetImportContext ctx, XElement map, string dir, int mapTileW, int mapTileH)
        {
            var tilesets = new List<TilesetInfo>();

            foreach (var tsElem in map.Elements("tileset"))
            {
                var firstGid = (int)tsElem.Attribute("firstgid");
                var source = (string)tsElem.Attribute("source");

                if (string.IsNullOrEmpty(source))
                {
                    ctx.LogImportWarning("Embedded tilesets are not supported; only external TSX files.");
                    continue;
                }

                var tsxPath = Path.Combine(dir, source).Replace('\\', '/');
                ctx.DependsOnSourceAsset(tsxPath);

                var tsxAbsPath = Path.GetFullPath(tsxPath);
                if (!File.Exists(tsxAbsPath))
                {
                    ctx.LogImportError($"TSX file not found: {source}");
                    return null;
                }

                var tsx = XDocument.Load(tsxAbsPath);
                var tsRoot = tsx.Root;
                if (tsRoot == null) continue;

                var tileCount = (int)tsRoot.Attribute("tilecount");
                var tileW = tsRoot.Attribute("tilewidth") != null
                    ? (int)tsRoot.Attribute("tilewidth")
                    : mapTileW;
                var tileH = tsRoot.Attribute("tileheight") != null
                    ? (int)tsRoot.Attribute("tileheight")
                    : mapTileH;

                var imageElem = tsRoot.Element("image");
                if (imageElem == null)
                {
                    ctx.LogImportError($"TSX {source} has no <image> element.");
                    return null;
                }

                var imgSrc = (string)imageElem.Attribute("source");
                var pngRelPath = Path.Combine(dir, imgSrc).Replace('\\', '/');
                ctx.DependsOnSourceAsset(pngRelPath);

                var pngAbsPath = Path.GetFullPath(pngRelPath);
                if (!File.Exists(pngAbsPath))
                {
                    ctx.LogImportError($"Tileset PNG not found: {imgSrc}");
                    return null;
                }

                var bytes = File.ReadAllBytes(pngAbsPath);
                var tex = new Texture2D(1, 1, TextureFormat.RGBA32, false)
                {
                    name = Path.GetFileNameWithoutExtension(imgSrc),
                    filterMode = filterMode,
                    wrapMode = TextureWrapMode.Clamp,
                };
                tex.LoadImage(bytes);
                ctx.AddObjectToAsset($"tileset_tex_{tilesets.Count}", tex);

                tilesets.Add(new TilesetInfo
                {
                    firstGid = firstGid,
                    tileCount = tileCount,
                    tileWidth = tileW,
                    tileHeight = tileH,
                    texture = tex,
                });
            }

            return tilesets;
        }

        private Dictionary<uint, Tile> BuildTileAssets(AssetImportContext ctx, List<TilesetInfo> tilesets)
        {
            var tiles = new Dictionary<uint, Tile>();

            foreach (var ts in tilesets)
            {
                for (int localId = 0; localId < ts.tileCount; localId++)
                {
                    uint gid = (uint)(ts.firstGid + localId);

                    // Tiles are packed in a single horizontal row; no Y offset.
                    var rect = new Rect(
                        localId * ts.tileWidth,
                        ts.texture.height - ts.tileHeight, // Y=0 at bottom in Unity textures
                        ts.tileWidth,
                        ts.tileHeight
                    );

                    var sprite = Sprite.Create(
                        ts.texture,
                        rect,
                        new Vector2(0.5f, 0.5f),
                        pixelsPerUnit,
                        0,
                        SpriteMeshType.FullRect
                    );
                    sprite.name = $"tile_{gid}";

                    var tile = ScriptableObject.CreateInstance<Tile>();
                    tile.name = $"tile_{gid}";
                    tile.sprite = sprite;

                    ctx.AddObjectToAsset($"tile_sprite_{gid}", sprite);
                    ctx.AddObjectToAsset($"tile_{gid}", tile);
                    tiles[gid] = tile;
                }
            }

            return tiles;
        }

        private void ImportLayer(
            AssetImportContext ctx,
            XElement layerElem,
            GameObject gridGO,
            int mapWidth,
            int mapHeight,
            Dictionary<uint, Tile> tileAssets,
            int layerIndex)
        {
            var layerName = (string)layerElem.Attribute("name") ?? $"Layer {layerIndex}";
            var dataElem = layerElem.Element("data");
            if (dataElem == null)
            {
                ctx.LogImportWarning($"Layer '{layerName}' has no <data> element; skipping.");
                return;
            }

            var encoding = (string)dataElem.Attribute("encoding");
            if (encoding != "csv")
            {
                ctx.LogImportWarning($"Layer '{layerName}' uses encoding '{encoding}'; only CSV is supported.");
                return;
            }

            var layerGO = new GameObject(layerName);
            layerGO.transform.SetParent(gridGO.transform, false);

            var tilemap = layerGO.AddComponent<Tilemap>();
            layerGO.AddComponent<TilemapRenderer>();

            var csv = dataElem.Value.Trim();
            var tokens = csv.Split(new[] { ',', '\n', '\r' }, StringSplitOptions.RemoveEmptyEntries);

            int col = 0;
            int row = 0;

            foreach (var token in tokens)
            {
                if (!uint.TryParse(token.Trim(), out uint rawGid))
                {
                    col++;
                    if (col >= mapWidth) { col = 0; row++; }
                    continue;
                }

                if (rawGid != 0)
                {
                    bool flipX = (rawGid & 0x80000000) != 0;
                    bool flipY = (rawGid & 0x40000000) != 0;
                    bool flipD = (rawGid & 0x20000000) != 0;
                    uint gid = rawGid & 0x1FFFFFFF;

                    // Tiled row 0 is at the top; Unity Tilemap row 0 is at the bottom.
                    var pos = new Vector3Int(col, mapHeight - 1 - row, 0);

                    if (tileAssets.TryGetValue(gid, out var tile))
                    {
                        tilemap.SetTile(pos, tile);

                        if (flipX || flipY || flipD)
                        {
                            var matrix = TileFlipMatrix(flipX, flipY, flipD);
                            tilemap.SetTransformMatrix(pos, matrix);
                        }
                    }
                }

                col++;
                if (col >= mapWidth) { col = 0; row++; }
            }
        }

        // Builds a transform matrix for the 8 possible Tiled flip/rotate combinations.
        // Diagonal flip in Tiled (Y-down) maps to 90° CCW rotation in Unity (Y-up).
        private static Matrix4x4 TileFlipMatrix(bool flipX, bool flipY, bool flipD)
        {
            int flags = (flipX ? 1 : 0) | (flipY ? 2 : 0) | (flipD ? 4 : 0);
            var s = Vector3.one;
            switch (flags)
            {
                case 1: // flip X
                    return Matrix4x4.Scale(new Vector3(-1, 1, 1));
                case 2: // flip Y
                    return Matrix4x4.Scale(new Vector3(1, -1, 1));
                case 3: // flip X + Y = 180°
                    return Matrix4x4.Scale(new Vector3(-1, -1, 1));
                case 4: // diagonal only = 90° CCW
                    return Matrix4x4.Rotate(Quaternion.Euler(0, 0, 90));
                case 5: // diagonal + X = 90° CW
                    return Matrix4x4.Rotate(Quaternion.Euler(0, 0, -90));
                case 6: // diagonal + Y = 90° CCW + flip X
                    return Matrix4x4.Rotate(Quaternion.Euler(0, 0, 90)) *
                           Matrix4x4.Scale(new Vector3(-1, 1, 1));
                case 7: // diagonal + X + Y = 90° CW + flip X
                    return Matrix4x4.Rotate(Quaternion.Euler(0, 0, -90)) *
                           Matrix4x4.Scale(new Vector3(-1, 1, 1));
                default:
                    return Matrix4x4.identity;
            }
        }
    }
}
