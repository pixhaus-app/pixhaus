---
title: "Motion from video"
description: "Extract motion timing from a reference video into the timeline."
---

Analyzes a reference video, extracts pose timing and keyframe positions, and populates the timeline with markers and rough silhouette pose layers. The pose layers are a timing reference, not finished art.

Invoke via `AI > Motion from video`. Drop an MP4, MOV, or GIF into the input.

**Backend:** Pose extraction (DensePose or MediaPipe) with VLM for keyframe identification.

