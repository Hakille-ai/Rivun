import type { Config } from "tailwindcss";

const config: Config = {
  content: [
    "./pages/**/*.{js,ts,jsx,tsx,mdx}",
    "./components/**/*.{js,ts,jsx,tsx,mdx}",
    "./app/**/*.{js,ts,jsx,tsx,mdx}",
    "./lib/**/*.{js,ts,jsx,tsx,mdx}",
  ],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        bg: {
          base: "#07090E",
          surface: "#0D111A",
          "surface-raised": "#131926",
          subtle: "#182030",
        },
        border: {
          subtle: "#1E293B",
          strong: "#334155",
          highlight: "#475569",
        },
        accent: {
          primary: "#38BDF8", // Cyan-400
          secondary: "#818CF8", // Indigo-400
          emerald: "#34D399", // Emerald-400
          amber: "#FBBF24", // Amber-400
          rose: "#FB7185", // Rose-400
          purple: "#C084FC", // Purple-400
        },
        status: {
          verified: "#34D399",
          "verified-bg": "rgba(52, 211, 153, 0.12)",
          warning: "#FBBF24",
          "warning-bg": "rgba(251, 191, 36, 0.12)",
          critical: "#F87171",
          "critical-bg": "rgba(248, 113, 113, 0.12)",
          info: "#38BDF8",
          "info-bg": "rgba(56, 189, 248, 0.12)",
        },
        text: {
          primary: "#F8FAFC",
          secondary: "#94A3B8",
          muted: "#64748B",
        },
      },
      fontFamily: {
        sans: ["Inter", "-apple-system", "BlinkMacSystemFont", "Segoe UI", "sans-serif"],
        mono: ["JetBrains Mono", "SF Mono", "Fira Code", "Consolas", "monospace"],
      },
      boxShadow: {
        card: "0 1px 3px 0 rgba(0, 0, 0, 0.5), 0 1px 2px -1px rgba(0, 0, 0, 0.5)",
        modal: "0 25px 50px -12px rgba(0, 0, 0, 0.85)",
        glow: "0 0 25px rgba(56, 189, 248, 0.2)",
        "glow-indigo": "0 0 25px rgba(129, 140, 248, 0.2)",
      },
      animation: {
        "pulse-subtle": "pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite",
        "accordion-down": "accordion-down 0.2s ease-out",
        "accordion-up": "accordion-up 0.2s ease-out",
      },
    },
  },
  plugins: [],
};

export default config;
