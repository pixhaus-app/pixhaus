/**
 * Pixhaus Tailwind config — shared preset for downstream sites.
 *
 * Designed to be imported via Tailwind's `presets: [...]` field, not used
 * directly. The `content` field is consumer-provided so paths resolve
 * against the consumer's source tree, not this brand directory. Example
 * for S47:
 *
 *   import pixhausPreset from "@pixhaus/brand/tailwind.config";
 *   export default { presets: [pixhausPreset], content: ["./src/**\/*"] };
 *
 * All color values mirror brand/tokens.css. When updating one, update both.
 */
import type { Config } from "tailwindcss";

const config: Omit<Config, "content"> = {
  darkMode: ["selector", '[data-theme="dark"]'],
  theme: {
    extend: {
      colors: {
        brand: {
          indigo: {
            50: "#f0eeff",
            100: "#e0dcff",
            200: "#c5bdfd",
            300: "#a497fb",
            400: "#9080f8",
            500: "#7c6cef",
            600: "#6555d4",
            700: "#5040b2",
            800: "#3b2e8a",
            900: "#271e5e",
          },
          neutral: {
            0: "#0f0f13",
            50: "#17171e",
            100: "#1e1e27",
            150: "#252530",
            200: "#2c2c3a",
            300: "#2a2a35",
            400: "#3d3d50",
            500: "#5a5a72",
            600: "#9090a8",
            700: "#b8b8cc",
            800: "#d8d8e8",
            900: "#e6e6f0",
            950: "#f0f0f4",
            1000: "#ffffff",
          },
        },
        /* Semantic aliases — CSS-variable-backed so Tailwind works alongside
           the custom-property theme system. */
        bg: {
          app: "var(--color-bg-app)",
          panel: "var(--color-bg-panel)",
          elevated: "var(--color-bg-elevated)",
          hover: "var(--color-bg-hover)",
          active: "var(--color-bg-active)",
          selection: "var(--color-bg-selection)",
        },
        border: {
          DEFAULT: "var(--color-border)",
          strong: "var(--color-border-strong)",
        },
        text: {
          primary: "var(--color-text-primary)",
          secondary: "var(--color-text-secondary)",
          disabled: "var(--color-text-disabled)",
          accent: "var(--color-text-on-accent)",
        },
        accent: {
          DEFAULT: "var(--color-accent)",
          hover: "var(--color-accent-hover)",
          subtle: "var(--color-accent-subtle)",
          border: "var(--color-accent-border)",
        },
        status: {
          success: "var(--color-success)",
          warning: "var(--color-warning)",
          error: "var(--color-error)",
          info: "var(--color-info)",
        },
      },

      fontFamily: {
        sans: [
          "Geist",
          "-apple-system",
          "BlinkMacSystemFont",
          '"Segoe UI"',
          "Roboto",
          "Helvetica",
          "Arial",
          "sans-serif",
        ],
        mono: [
          '"Geist Mono"',
          '"JetBrains Mono"',
          '"Cascadia Code"',
          '"Fira Code"',
          "Menlo",
          "Monaco",
          "Consolas",
          "monospace",
        ],
      },

      fontSize: {
        xs: ["10px", { lineHeight: "1.4" }],
        sm: ["11px", { lineHeight: "1.4" }],
        base: ["13px", { lineHeight: "1.5" }],
        md: ["14px", { lineHeight: "1.5" }],
        lg: ["16px", { lineHeight: "1.5" }],
        xl: ["20px", { lineHeight: "1.35" }],
        "2xl": ["24px", { lineHeight: "1.2" }],
        "3xl": ["28px", { lineHeight: "1.2" }],
        "4xl": ["36px", { lineHeight: "1.1" }],
      },

      fontWeight: {
        regular: "400",
        medium: "500",
        semibold: "600",
        bold: "700",
      },

      letterSpacing: {
        tight: "-0.03em",
        snug: "-0.01em",
        normal: "0em",
        wide: "0.05em",
        wider: "0.08em",
        widest: "0.12em",
      },

      borderRadius: {
        sm: "3px",
        md: "5px",
        DEFAULT: "5px",
        lg: "8px",
        xl: "12px",
        full: "9999px",
      },

      spacing: {
        1: "2px",
        2: "4px",
        3: "6px",
        4: "8px",
        5: "10px",
        6: "12px",
        7: "14px",
        8: "16px",
        10: "20px",
        12: "24px",
        16: "32px",
        20: "40px",
        24: "48px",
        32: "64px",
      },

      transitionDuration: {
        fast: "80ms",
        mid: "150ms",
        slow: "300ms",
      },

      transitionTimingFunction: {
        enter: "cubic-bezier(0.16, 1, 0.3, 1)",
        exit: "cubic-bezier(0.4, 0, 1, 1)",
      },

      zIndex: {
        base: "0",
        raised: "10",
        dropdown: "100",
        modal: "500",
        overlay: "900",
        toast: "1000",
        tooltip: "1100",
      },

      boxShadow: {
        panel: "0 2px 8px rgba(0, 0, 0, 0.3)",
        modal: "0 8px 24px rgba(0, 0, 0, 0.4), 0 2px 8px rgba(0, 0, 0, 0.3)",
        dialog: "0 12px 32px rgba(0, 0, 0, 0.4), 0 2px 8px rgba(0, 0, 0, 0.2)",
      },
    },
  },
  plugins: [],
};

export default config;
