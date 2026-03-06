/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    './src/app/**/*.{js,ts,jsx,tsx,mdx}',
    './src/components/**/*.{js,ts,jsx,tsx,mdx}',
    './src/hooks/**/*.{js,ts,jsx,tsx,mdx}',
    './src/lib/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {
      colors: {
        base: {
          950: '#091540',
          900: '#11205d',
          800: '#1b2cc1',
          700: '#3d518c',
        },
        ink: {
          50: '#f5f9ff',
          100: '#deebff',
          200: '#abd2fa',
          300: '#7692ff',
          400: '#7a90d8',
        },
        accent: {
          400: '#abd2fa',
          500: '#7692ff',
          600: '#1b2cc1',
        },
        ring: {
          DEFAULT: '#3d518c',
          soft: '#1b2cc1',
        },
      },
      fontFamily: {
        sans: ['var(--font-sans)'],
        mono: ['var(--font-mono)'],
      },
      boxShadow: {
        panel: '0 22px 70px rgba(9, 21, 64, 0.42)',
      },
      keyframes: {
        pulseSoft: {
          '0%, 100%': { opacity: '0.6' },
          '50%': { opacity: '1' },
        },
      },
      animation: {
        pulseSoft: 'pulseSoft 1.5s ease-in-out infinite',
      },
    },
  },
  plugins: [],
}
