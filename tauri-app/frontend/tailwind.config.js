/** @type {import('tailwindcss').Config} */
export default {
  darkMode: 'class',
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        border: 'hsl(var(--border))',
        input: 'hsl(var(--input))',
        ring: 'hsl(var(--ring))',
        background: 'hsl(var(--background))',
        foreground: 'hsl(var(--foreground))',
        primary: {
          DEFAULT: 'hsl(var(--primary))',
          foreground: 'hsl(var(--primary-foreground))',
        },
        secondary: {
          DEFAULT: 'hsl(var(--secondary))',
          foreground: 'hsl(var(--secondary-foreground))',
        },
        destructive: {
          DEFAULT: 'hsl(var(--destructive))',
          foreground: 'hsl(var(--destructive-foreground))',
        },
        muted: {
          DEFAULT: 'hsl(var(--muted))',
          foreground: 'hsl(var(--muted-foreground))',
        },
        accent: {
          DEFAULT: 'hsl(var(--accent))',
          foreground: 'hsl(var(--accent-foreground))',
        },
        popover: {
          DEFAULT: 'hsl(var(--popover))',
          foreground: 'hsl(var(--popover-foreground))',
        },
        card: {
          DEFAULT: 'hsl(var(--card))',
          foreground: 'hsl(var(--card-foreground))',
        },
        success: {
          DEFAULT: 'hsl(var(--success))',
          foreground: 'hsl(var(--success-foreground))',
        },
        warning: {
          DEFAULT: 'hsl(var(--warning))',
          foreground: 'hsl(var(--warning-foreground))',
        },
        info: {
          DEFAULT: 'hsl(var(--info))',
          foreground: 'hsl(var(--info-foreground))',
        },
      },
      borderRadius: {
        lg: 'var(--radius)',
        md: 'calc(var(--radius) - 2px)',
        sm: 'calc(var(--radius) - 4px)',
      },
      fontFamily: {
        sans: ['Inter', 'system-ui', '-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'Roboto', 'sans-serif'],
        mono: ['JetBrains Mono', 'Fira Code', 'Consolas', 'monospace'],
      },
      animation: {
        'fade-in': 'fadeIn 0.3s ease-out',
        'slide-up': 'slideUp 0.4s ease-out',
        'slide-down': 'slideDown 0.3s ease-out',
        'scale-in': 'scaleIn 0.2s ease-out',
        'pulse-soft': 'pulseSoft 2s ease-in-out infinite',
        'spin-slow': 'spin 3s linear infinite',
        'bounce-soft': 'bounceSoft 2s ease-in-out infinite',
        'shimmer': 'shimmer 2s linear infinite',
        'refresh-hover': 'refreshHover 0.5s ease-in-out',
        'refresh-spin': 'refreshSpin 0.8s linear infinite',
        'icon-hover-rotate': 'iconHoverRotate 0.4s ease-in-out',
        'icon-hover-wiggle': 'iconHoverWiggle 0.4s ease-in-out',
        'icon-hover-flyout': 'iconHoverFlyout 0.5s ease-in-out',
        'status-flash': 'statusFlash 0.6s ease-out',
        'status-breathe': 'statusBreathe 2s ease-in-out infinite',
        'log-flash': 'logFlash 0.8s ease-out',
        'bg-shift': 'bgShift 20s ease-in-out infinite',
      },
      keyframes: {
        fadeIn: {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' },
        },
        slideUp: {
          '0%': { opacity: '0', transform: 'translateY(12px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        slideDown: {
          '0%': { opacity: '0', transform: 'translateY(-8px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        scaleIn: {
          '0%': { opacity: '0', transform: 'scale(0.95)' },
          '100%': { opacity: '1', transform: 'scale(1)' },
        },
        pulseSoft: {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.6' },
        },
        bounceSoft: {
          '0%, 100%': { transform: 'translateY(0)' },
          '50%': { transform: 'translateY(-4px)' },
        },
        shimmer: {
          '0%': { backgroundPosition: '-200% 0' },
          '100%': { backgroundPosition: '200% 0' },
        },
        refreshHover: {
          '0%': { transform: 'rotate(0deg)' },
          '100%': { transform: 'rotate(180deg)' },
        },
        iconHoverRotate: {
          '0%': { transform: 'rotate(0deg)' },
          '100%': { transform: 'rotate(180deg)' },
        },
        iconHoverWiggle: {
          '0%, 100%': { transform: 'rotate(0deg)' },
          '25%': { transform: 'rotate(-12deg)' },
          '75%': { transform: 'rotate(12deg)' },
        },
        iconHoverFlyout: {
          '0%': { transform: 'translate(0, 0) scale(1)', opacity: '1' },
          '40%': { transform: 'translate(3px, -3px) scale(1.3)', opacity: '0.7' },
          '100%': { transform: 'translate(0, 0) scale(1)', opacity: '1' },
        },
        refreshSpin: {
          '0%': { transform: 'rotate(0deg)' },
          '100%': { transform: 'rotate(360deg)' },
        },
        statusFlash: {
          '0%': { filter: 'brightness(1)' },
          '30%': { filter: 'brightness(1.5)' },
          '60%': { filter: 'brightness(0.9)' },
          '100%': { filter: 'brightness(1)' },
        },
        statusBreathe: {
          '0%, 100%': { opacity: '1' },
          '50%': { opacity: '0.6' },
        },
        logFlash: {
          '0%': { backgroundColor: 'transparent' },
          '20%': { backgroundColor: 'hsl(var(--primary) / 0.08)' },
          '100%': { backgroundColor: 'transparent' },
        },
        bgShift: {
          '0%, 100%': { transform: 'translate(0%, 0%) rotate(0deg)' },
          '25%': { transform: 'translate(-5%, 3%) rotate(1deg)' },
          '50%': { transform: 'translate(3%, -5%) rotate(-1deg)' },
          '75%': { transform: 'translate(-3%, -2%) rotate(0.5deg)' },
        },
      },
      backdropBlur: {
        xs: '2px',
      },
    },
  },
  plugins: [],
}
