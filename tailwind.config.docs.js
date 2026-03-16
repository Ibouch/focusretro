/** @type {import('tailwindcss').Config} */
export default {
  content: ["./docs/index.html"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        brand: {
          50: "#FFF9E6",
          100: "#FFEFC2",
          200: "#FFE39A",
          300: "#FFD366",
          400: "#FFC338",
          500: "#F6A800",
          600: "#D18A00",
          700: "#A06100",
          800: "#6C3F00",
          900: "#3E2500",
        },
      },
    },
  },
  plugins: [],
};
