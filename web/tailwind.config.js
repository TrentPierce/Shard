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
          950: '#07131d',
          900: '#0d1b29',
          800: '#13283a',
          700: '#1b3850',
        },
        ink: {
          50: '#f6fbff',
          100: '#d7e6f2',
          200: '#aec5d6',
          300: '#7e96a8',
          400: '#5f7587',
        },
        accent: {
          400: '#78f0e1',
          500: '#33d0c1',
          600: '#1f9387',
        },
        ring: {
          DEFAULT: '#1f9387',
          soft: '#173845',
        },
      },
      fontFamily: {
        sans: ['var(--font-sans)'],
        mono: ['var(--font-mono)'],
      },
      boxShadow: {
        panel: '0 24px 80px rgba(3, 10, 16, 0.42)',
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
