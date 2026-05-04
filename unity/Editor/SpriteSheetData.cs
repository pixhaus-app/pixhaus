using System;

namespace Pixhaus.Editor
{
    // C# model for the Pixhaus sprite sheet JSON format.
    // Schema is documented in docs/unity-handoff.md.
    // JsonUtility deserializes reference-type fields to null when absent in the JSON,
    // so optional nested objects (SliceKey.pivot, SliceKey.center) are safe to null-check.

    [Serializable]
    internal class SpriteSheetData
    {
        public FrameData[] frames;
        public MetaData meta;
    }

    [Serializable]
    internal class FrameData
    {
        public string filename;
        public RectData frame;
        public bool rotated;
        public bool trimmed;
        public RectData spriteSourceSize;
        public SizeData sourceSize;
        public int duration; // milliseconds
    }

    [Serializable]
    internal class MetaData
    {
        public string app;
        public string version;
        public string image;   // PNG filename, no path component
        public string format;  // always "RGBA8888"
        public SizeData size;
        public string scale;
        public FrameTag[] frameTags;
        public LayerData[] layers;
        public SliceData[] slices;
    }

    [Serializable]
    internal class RectData
    {
        public int x, y, w, h;
    }

    [Serializable]
    internal class SizeData
    {
        public int w, h;
    }

    [Serializable]
    internal class FrameTag
    {
        public string name;
        public int from;         // first frame index, inclusive
        public int to;           // last frame index, inclusive
        public string direction; // "forward" | "reverse" | "pingpong" | "pingpong_reverse"
        public int repeat;       // 0 = loop forever; >0 = N cycles
        public string color;     // "#RRGGBBAA"
    }

    [Serializable]
    internal class LayerData
    {
        public string name;
        public int opacity;
        public string blendMode;
    }

    [Serializable]
    internal class SliceData
    {
        public string name;
        public string color;
        public SliceKey[] keys;
    }

    [Serializable]
    internal class SliceKey
    {
        public int frame;
        public RectData bounds;
        public RectData center;  // optional: nine-slice center patch
        public PivotData pivot;  // optional: pivot offset from bounds.origin
    }

    [Serializable]
    internal class PivotData
    {
        public int x;
        public int y;
    }
}
