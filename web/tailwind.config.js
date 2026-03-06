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
          950: '#2c302e',
          900: '#363a38',
          800: '#474a48',
          700: '#537a5a',
        },
        ink: {
          50: '#f2f5f2',
          100: '#d7ddd7',
          200: '#b9c3b9',
          300: '#909590',
          400: '#7d857e',
        },
        accent: {
          400: '#9ae19d',
          500: '#7ac67d',
          600: '#537a5a',
        },
        ring: {
          DEFAULT: '#537a5a',
          soft: '#474a48',
        },
      },
      fontFamily: {
        sans: ['var(--font-sans)'],
        mono: ['var(--font-mono)'],
      },
      boxShadow: {
        panel: '0 22px 70px rgba(22, 25, 23, 0.38)',
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
