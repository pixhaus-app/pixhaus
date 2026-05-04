using System.Collections.Generic;
using System.IO;
using UnityEditor;
using UnityEditor.AssetImporters;
using UnityEngine;

namespace Pixhaus.Editor
{
    // ScriptedImporter for .pixhaussprite files.
    //
    // A .pixhaussprite file is the Pixhaus sprite sheet JSON (docs/unity-handoff.md)
    // renamed to this extension so Unity uses this importer automatically. The co-located
    // PNG (named in meta.image) is loaded from disk and declared as a source dependency.
    //
    // Produces per-import:
    //   - Texture2D  (main asset, the packed sprite sheet)
    //   - Sprite     per frame (sub-assets)
    //   - AnimationClip per frame tag (sub-assets, for use with Unity's Animator)
    //
    // For scripted playback without an Animator, use the PixhausAnimator component.

    [ScriptedImporter(1, "pixhaussprite")]
    public class PixhausSpriteImporter : ScriptedImporter
    {
        [SerializeField] public float pixelsPerUnit = 16f;
        [SerializeField] public FilterMode filterMode = FilterMode.Point;
        [SerializeField] public bool generateMipMaps = false;
        [SerializeField] public SpriteMeshType spriteMeshType = SpriteMeshType.FullRect;

        public override void OnImportAsset(AssetImportContext ctx)
        {
            var jsonText = File.ReadAllText(ctx.assetPath);
            var data = JsonUtility.FromJson<SpriteSheetData>(jsonText);

            if (data?.meta == null || data.frames == null)
            {
                ctx.LogImportError("Failed to parse Pixhaus sprite sheet JSON. Check that the file is valid.");
                return;
            }

            var assetDir = Path.GetDirectoryName(ctx.assetPath);
            var pngRelPath = Path.Combine(assetDir, data.meta.image)
                .Replace('\\', '/');

            ctx.DependsOnSourceAsset(pngRelPath);

            var pngAbsPath = Path.GetFullPath(pngRelPath);
            if (!File.Exists(pngAbsPath))
            {
                ctx.LogImportError($"PNG not found: {data.meta.image} (resolved to {pngAbsPath})");
                return;
            }

            var texture = LoadTexture(pngAbsPath, Path.GetFileNameWithoutExtension(ctx.assetPath));
            ctx.AddObjectToAsset("texture", texture);
            ctx.SetMainObject(texture);

            var sprites = CreateSprites(ctx, data, texture);
            CreateAnimationClips(ctx, data, sprites);
        }

        private Texture2D LoadTexture(string absolutePath, string name)
        {
            var bytes = File.ReadAllBytes(absolutePath);
            var tex = new Texture2D(1, 1, TextureFormat.RGBA32, generateMipMaps)
            {
                name = name,
                filterMode = filterMode,
                wrapMode = TextureWrapMode.Clamp,
            };
            tex.LoadImage(bytes); // flips Y to Unity convention (Y=0 at bottom)
            return tex;
        }

        private Sprite[] CreateSprites(AssetImportContext ctx, SpriteSheetData data, Texture2D texture)
        {
            var frames = data.frames;
            var sprites = new Sprite[frames.Length];

            for (int i = 0; i < frames.Length; i++)
            {
                var f = frames[i];
                var pivot = ResolvePivot(data, i, f);

                // Convert from PNG coordinates (Y=0 at top) to Unity texture coordinates (Y=0 at bottom).
                var rect = new Rect(
                    f.frame.x,
                    texture.height - f.frame.y - f.frame.h,
                    f.frame.w,
                    f.frame.h
                );

                var sprite = Sprite.Create(texture, rect, pivot, pixelsPerUnit, 0, spriteMeshType);
                sprite.name = f.filename;
                sprites[i] = sprite;
                ctx.AddObjectToAsset($"sprite_{i}", sprite);
            }

            return sprites;
        }

        // Resolve the normalized pivot for a frame.
        //
        // Looks for a "root" slice with a pivot key that covers this frame index.
        // Falls back to bottom-center (0.5, 0) which works for most standing characters.
        private Vector2 ResolvePivot(SpriteSheetData data, int frameIndex, FrameData frameData)
        {
            if (data.meta.slices != null)
            {
                foreach (var slice in data.meta.slices)
                {
                    if (slice.name != "root" || slice.keys == null)
                        continue;

                    // Keys are ordered by frame; find the last key whose frame <= frameIndex.
                    SliceKey applicable = null;
                    foreach (var key in slice.keys)
                    {
                        if (key.frame <= frameIndex)
                            applicable = key;
                    }

                    if (applicable?.pivot == null)
                        continue;

                    // bounds and pivot are in per-frame canvas space (Y=0 at top).
                    var canvasX = applicable.bounds.x + applicable.pivot.x;
                    var canvasY = applicable.bounds.y + applicable.pivot.y;

                    // Normalize relative to the frame dimensions, flipping Y for Unity (Y=0 at bottom).
                    var nx = (float)(canvasX - 0) / frameData.sourceSize.w;
                    var ny = 1f - (float)canvasY / frameData.sourceSize.h;
                    return new Vector2(nx, ny);
                }
            }

            return new Vector2(0.5f, 0f); // bottom-center
        }

        private void CreateAnimationClips(AssetImportContext ctx, SpriteSheetData data, Sprite[] sprites)
        {
            if (data.meta.frameTags == null)
                return;

            foreach (var tag in data.meta.frameTags)
            {
                var clip = BuildClip(tag, data.frames, sprites);
                ctx.AddObjectToAsset($"clip_{tag.name}", clip);
            }
        }

        private AnimationClip BuildClip(FrameTag tag, FrameData[] frames, Sprite[] sprites)
        {
            var clip = new AnimationClip { name = tag.name };

            var binding = new EditorCurveBinding
            {
                type = typeof(SpriteRenderer),
                path = "",
                propertyName = "m_Sprite",
            };

            bool reverse = tag.direction == "reverse" || tag.direction == "pingpong_reverse";
            bool pingpong = tag.direction == "pingpong" || tag.direction == "pingpong_reverse";

            var keyframes = BuildKeyframes(tag, frames, sprites, reverse);
            AnimationUtility.SetObjectReferenceCurve(clip, binding, keyframes);

            var settings = AnimationUtility.GetAnimationClipSettings(clip);
            settings.loopTime = !pingpong && tag.repeat == 0;
            AnimationUtility.SetAnimationClipSettings(clip, settings);

            if (pingpong)
                clip.wrapMode = WrapMode.PingPong;
            else if (tag.repeat == 0)
                clip.wrapMode = WrapMode.Loop;
            else
                clip.wrapMode = WrapMode.Once;

            return clip;
        }

        private static ObjectReferenceKeyframe[] BuildKeyframes(
            FrameTag tag, FrameData[] frames, Sprite[] sprites, bool reverse)
        {
            var kfs = new List<ObjectReferenceKeyframe>();
            float time = 0f;

            int start = reverse ? tag.to : tag.from;
            int end = reverse ? tag.from : tag.to;
            int step = reverse ? -1 : 1;

            for (int i = start; reverse ? i >= end : i <= end; i += step)
            {
                kfs.Add(new ObjectReferenceKeyframe { time = time, value = sprites[i] });
                time += frames[i].duration / 1000f;
            }

            // Trailing keyframe defines clip length and loops back to the first frame.
            if (kfs.Count > 0)
                kfs.Add(new ObjectReferenceKeyframe { time = time, value = kfs[0].value });

            return kfs.ToArray();
        }
    }
}
