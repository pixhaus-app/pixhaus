import { defineConfig } from "astro/config";
import sitemap from "@astrojs/sitemap";
import { fileURLToPath } from "url";
import { dirname, resolve } from "path";

const __dirname = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  site: "https://pixhaus.app",
  integrations: [
    sitemap({
      filter: (page) => !page.includes("/og/"),
    }),
  ],
  vite: {
    resolve: {
      alias: {
        "@brand": resolve(__dirname, "../brand"),
      },
    },
  },
});
