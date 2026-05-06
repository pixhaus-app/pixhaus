/**
 * Generates SVG-based Open Graph images for each page.
 * Served at /og/<page>.png via Astro's static file generation.
 * The SVG is returned with image/svg+xml content-type; social crawlers
 * accept SVG for og:image on most platforms.
 */

export function getStaticPaths() {
  return [
    { params: { page: "home" } },
    { params: { page: "download" } },
    { params: { page: "features" } },
    { params: { page: "compare" } },
    { params: { page: "community" } },
    { params: { page: "blog" } },
  ];
}

const subtitles: Record<string, string> = {
  home: "Open-source AI-native pixel art editor for sprites, animations, and tilemaps.",
  download: "Windows, macOS, and Linux. Free forever. MIT license.",
  features: "Editing, animation, tilemaps, and 14 AI verbs built in.",
  compare: "Pixhaus vs Aseprite, Pixelorama, and Photoshop.",
  community: "Open source, built in public. GitHub and Discord.",
  blog: "Development updates, release notes, and design decisions.",
};

const titles: Record<string, string> = {
  home: "Pixhaus",
  download: "Download",
  features: "Features",
  compare: "Compare",
  community: "Community",
  blog: "Blog",
};

export async function GET({ params }: { params: { page: string } }) {
  const page = params.page;
  const title = titles[page] ?? "Pixhaus";
  const subtitle = subtitles[page] ?? "";

  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1200" y2="630" gradientUnits="userSpaceOnUse">
      <stop offset="0%" stop-color="#0f0f13"/>
      <stop offset="100%" stop-color="#1e1e27"/>
    </linearGradient>
    <radialGradient id="glow" cx="60%" cy="30%" r="60%" gradientUnits="userSpaceOnUse">
      <stop offset="0%" stop-color="#7c6cef" stop-opacity="0.2"/>
      <stop offset="100%" stop-color="transparent" stop-opacity="0"/>
    </radialGradient>
  </defs>

  <!-- Background -->
  <rect width="1200" height="630" fill="url(#bg)"/>
  <rect width="1200" height="630" fill="url(#glow)"/>

  <!-- Grid pattern -->
  <g stroke="#2c2c3a" stroke-width="0.5" opacity="0.4">
    ${Array.from({ length: 20 }, (_, i) => `<line x1="${i * 64}" y1="0" x2="${i * 64}" y2="630"/>`).join("")}
    ${Array.from({ length: 10 }, (_, i) => `<line x1="0" y1="${i * 70}" x2="1200" y2="${i * 70}"/>`).join("")}
  </g>

  <!-- Symbol (top-left) -->
  <g transform="translate(80, 80) scale(0.6)" fill="#7c6cef">
    <rect x="41" y="1" width="18" height="18" rx="2"/>
    <rect x="21" y="21" width="18" height="18" rx="2"/>
    <rect x="41" y="21" width="18" height="18" rx="2"/>
    <rect x="61" y="21" width="18" height="18" rx="2"/>
    <rect x="1"  y="41" width="18" height="18" rx="2"/>
    <rect x="21" y="41" width="18" height="18" rx="2"/>
    <rect x="41" y="41" width="18" height="18" rx="2"/>
    <rect x="61" y="41" width="18" height="18" rx="2"/>
    <rect x="81" y="41" width="18" height="18" rx="2"/>
    <rect x="1"  y="61" width="18" height="18" rx="2"/>
    <rect x="21" y="61" width="18" height="18" rx="2"/>
    <rect x="61" y="61" width="18" height="18" rx="2"/>
    <rect x="81" y="61" width="18" height="18" rx="2"/>
    <rect x="1"  y="81" width="18" height="18" rx="2"/>
    <rect x="21" y="81" width="18" height="18" rx="2"/>
    <rect x="61" y="81" width="18" height="18" rx="2"/>
    <rect x="81" y="81" width="18" height="18" rx="2"/>
  </g>

  <!-- Wordmark next to symbol -->
  <text x="150" y="132" font-family="Geist, -apple-system, sans-serif" font-size="32" font-weight="600" letter-spacing="-0.03em" fill="#9090a8">pixhaus</text>

  <!-- Title -->
  <text
    x="80"
    y="340"
    font-family="Geist, -apple-system, sans-serif"
    font-size="${title.length > 8 ? "72" : "88"}"
    font-weight="700"
    letter-spacing="-0.04em"
    fill="#e6e6f0"
  >${title}</text>

  <!-- Subtitle -->
  <text
    x="80"
    y="410"
    font-family="Geist, -apple-system, sans-serif"
    font-size="24"
    font-weight="400"
    fill="#5a5a72"
  >${subtitle}</text>

  <!-- Bottom bar -->
  <rect x="80" y="540" width="60" height="4" rx="2" fill="#7c6cef" opacity="0.8"/>
  <text x="80" y="585" font-family="Geist, -apple-system, sans-serif" font-size="18" fill="#5a5a72">pixhaus.app</text>
</svg>`;

  return new Response(svg, {
    headers: {
      "Content-Type": "image/svg+xml",
      "Cache-Control": "public, max-age=31536000, immutable",
    },
  });
}
