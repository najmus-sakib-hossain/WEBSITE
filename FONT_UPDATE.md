# JetBrains Mono Font Implementation

## Summary
Successfully replaced Hedvig Letters Sans and Hedvig Letters Serif fonts with JetBrains Mono throughout the entire website.

## Changes Made

### 1. Layout Configuration (`midday/apps/website/src/app/layout.tsx`)
- Replaced `Hedvig_Letters_Sans` and `Hedvig_Letters_Serif` imports with `JetBrains_Mono`
- Updated font configuration:
  - Weight: 400, 500, 600, 700 (multiple weights for flexibility)
  - Display: swap (better performance)
  - Variable: `--font-jetbrains-mono`
  - Fallback: monospace, Courier New
- Updated body className to use `font-mono` instead of `font-sans`

### 2. Global CSS (`midday/apps/website/src/styles/globals.css`)
- Updated CSS variable from `--font-hedvig-sans` and `--font-hedvig-serif` to `--font-jetbrains-mono`
- Font family: "JetBrains Mono", monospace, "Courier New"

### 3. Tailwind Config (`midday/packages/ui/tailwind.config.ts`)
- Updated all font families (sans, mono, serif) to use `var(--font-jetbrains-mono)`
- This ensures all Tailwind utility classes (font-sans, font-mono, font-serif) use JetBrains Mono

### 4. OG Image Route (`midday/apps/website/src/app/api/og/compare/route.tsx`)
- Updated font fetch URL to use JetBrains Mono from Google Fonts CDN
- Changed font name from "hedvig-sans" to "JetBrains Mono"
- Updated inline style to use "JetBrains Mono"

## Result
The entire website now uses JetBrains Mono as the primary font family across:
- All text content
- Headers and titles
- Body text
- Code snippets
- OG images
- All components using font-sans, font-mono, or font-serif classes

## Font Characteristics
JetBrains Mono is a monospace font designed for developers with:
- Clear character distinction
- Ligature support
- Optimized for code readability
- Multiple weights (400, 500, 600, 700)
- Professional, technical aesthetic

## Next Steps
To see the changes:
1. Restart the development server
2. Clear browser cache
3. The font will load from Google Fonts CDN automatically
