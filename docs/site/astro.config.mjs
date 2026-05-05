import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

export default defineConfig({
  integrations: [
    starlight({
      title: "Pixhaus",
      description:
        "Open-source AI-native pixel art editor for sprites, animations, and tilemaps.",
      logo: {
        light: "./src/assets/logo-light.svg",
        dark: "./src/assets/logo-dark.svg",
        replacesTitle: false,
      },
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: "https://github.com/pixhaus-app/pixhaus",
        },
      ],
      editLink: {
        baseUrl:
          "https://github.com/pixhaus-app/pixhaus/edit/main/docs/site/",
      },
      sidebar: [
        {
          label: "Getting Started",
          autogenerate: { directory: "getting-started" },
        },
        {
          label: "Editor",
          autogenerate: { directory: "editor" },
        },
        {
          label: "Animation",
          autogenerate: { directory: "animation" },
        },
        {
          label: "Tilemaps",
          autogenerate: { directory: "tilemaps" },
        },
        {
          label: "AI Verbs",
          autogenerate: { directory: "ai-verbs" },
        },
        {
          label: "Scripting",
          autogenerate: { directory: "scripting" },
        },
        {
          label: "Plugins",
          autogenerate: { directory: "plugins" },
        },
        {
          label: "Reference",
          autogenerate: { directory: "reference" },
        },
        {
          label: "FAQ",
          autogenerate: { directory: "faq" },
        },
      ],
      customCss: ["./src/styles/custom.css"],
    }),
  ],
  site: "https://docs.pixhaus.app",
});
