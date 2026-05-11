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
          items: [{ autogenerate: { directory: "getting-started" } }],
        },
        {
          label: "Editor",
          items: [{ autogenerate: { directory: "editor" } }],
        },
        {
          label: "Animation",
          items: [{ autogenerate: { directory: "animation" } }],
        },
        {
          label: "Tilemaps",
          items: [{ autogenerate: { directory: "tilemaps" } }],
        },
        {
          label: "AI Verbs",
          items: [{ autogenerate: { directory: "ai-verbs" } }],
        },
        {
          label: "Scripting",
          items: [{ autogenerate: { directory: "scripting" } }],
        },
        {
          label: "Plugins",
          items: [{ autogenerate: { directory: "plugins" } }],
        },
        {
          label: "Migration",
          items: [{ autogenerate: { directory: "migration" } }],
        },
        {
          label: "Reference",
          items: [{ autogenerate: { directory: "reference" } }],
        },
        {
          label: "FAQ",
          items: [{ autogenerate: { directory: "faq" } }],
        },
      ],
      customCss: ["./src/styles/custom.css"],
    }),
  ],
  site: "https://docs.pixhaus.app",
});
