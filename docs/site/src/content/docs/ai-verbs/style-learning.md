---
title: "Project style learning"
description: "Train a per-project style LoRA from existing layers."
---

Trains a small LoRA on every unique frame in the project, then registers it as the default style reference for subsequent verbs. Once trained, other verbs include the style automatically.

Invoke via `AI > Style learning`. Requires at least 20 distinct frames. Training takes 15-30 minutes on Replicate. The model file is stored in the project folder and reused for free on subsequent runs.

**Backend:** LoRA training via Replicate (cloud) or local Diffusers.

