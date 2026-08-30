import type { Config } from "tailwindcss";

const config: Config = {
  content: [
    "./pages/**/*.{js,ts,jsx,tsx,mdx}",
    "./components/**/*.{js,ts,jsx,tsx,mdx}",
    "./app/**/*.{js,ts,jsx,tsx,mdx}",
  ],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        bg: {
          base: "#0A0B0D",
          surface: "#111318",
          "surface-raised": "#181B22",
          subtle: "#14171F",
        },
        border: {
          subtle: "#22262F",
          strong: "#2E3440",
          highlight: "#3A4150",
        },
        accent: {
          primary: "#5B8CFF",
          hover: "#4378F0",
          glow: "rgba(91, 140, 255, 0.15)",
        },
        status: {
          verified: "#3DD68C",
          "verified-bg": "rgba(61, 214, 140, 0.1)",
          warning: "#E8B339",
          "warning-bg": "rgba(232, 179, 57, 0.1)",
          critical: "#F0554D",
          "critical-bg": "rgba(240, 85, 77, 0.1)",
        },
        text: {
          primary: "#F4F5F7",
          secondary: "#9AA1AE",
          muted: "#6B7280",
        },
      },
      fontFamily: {
        sans: ["Inter", "-apple-system", "BlinkMacSystemFont", "Segoe UI", "sans-serif"],
        mono: ["JetBrains Mono", "SF Mono", "Fira Code", "monospace"],
      },
      boxShadow: {
        card: "0 1px 3px 0 rgba(0, 0, 0, 0.4), 0 1px 2px -1px rgba(0, 0, 0, 0.4)",
        modal: "0 20px 25px -5px rgba(0, 0, 0, 0.6), 0 8px 10px -6px rgba(0, 0, 0, 0.6)",
        glow: "0 0 20px rgba(91, 140, 255, 0.25)",
      },
      animation: {
        pulse_subtle: "pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite",
      },
    },
  },
  plugins: [],
};
export default config;
