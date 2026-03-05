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
          950: '#191308',
          900: '#322a26',
          800: '#454b66',
          700: '#677db7',
        },
        ink: {
          50: '#f2f4ff',
          100: '#dde2ff',
          200: '#bcc3f2',
          300: '#9ca3db',
          400: '#677db7',
        },
        accent: {
          400: '#9ca3db',
          500: '#677db7',
          600: '#454b66',
        },
        ring: {
          DEFAULT: '#677db7',
          soft: '#454b66',
        },
      },
      fontFamily: {
        sans: ['var(--font-sans)'],
        mono: ['var(--font-mono)'],
      },
      boxShadow: {
        panel: '0 20px 50px rgba(25, 19, 8, 0.55)',
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
