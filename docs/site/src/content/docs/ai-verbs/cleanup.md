---
title: "Cleanup"
description: "Snap to palette, remove anti-aliasing, fix pivot drift."
---

Post-processes a layer to snap pixels to the project palette, remove sub-pixel anti-aliasing, and fix pivot drift across animation frames.

Invoke via `AI > Cleanup`. Recommended as a final pass after Inbetween or Continue, and after importing AI-generated or PSD content.

**Backend:** Classical image processing with lightweight VLM for ambiguous decisions.

