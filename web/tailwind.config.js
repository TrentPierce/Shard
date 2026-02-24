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
          950: '#05070a',
          900: '#0a0f16',
          800: '#111926',
          700: '#1b2638',
        },
        ink: {
          50: '#e7edf5',
          100: '#ced9e8',
          300: '#95a6c0',
          400: '#6f839e',
        },
        accent: {
          400: '#48d58f',
          500: '#30b977',
          600: '#20945f',
        },
        ring: {
          DEFAULT: '#223043',
          soft: '#1a2636',
        },
      },
      fontFamily: {
        sans: ['var(--font-sans)'],
        mono: ['var(--font-mono)'],
      },
      boxShadow: {
        panel: '0 20px 50px rgba(2, 6, 15, 0.45)',
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
