// Starlight 0.35+ uses Astro's Content Layer API. The legacy
// `type: "content"` collection silently produced an empty docs site
// (1 page, no sitemap entries) after the Astro 5→6 + Starlight bump;
// this config switches to the docsLoader so all the .md/.mdx files
// under src/content/docs/ are picked up again.
import { defineCollection } from "astro:content";
import { docsLoader } from "@astrojs/starlight/loaders";
import { docsSchema } from "@astrojs/starlight/schema";

export const collections = {
  docs: defineCollection({ loader: docsLoader(), schema: docsSchema() }),
};
