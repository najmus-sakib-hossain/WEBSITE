"use client";

import { cn } from "@midday/ui/cn";
import Link from "next/link";

type FooterLink = {
  href: string;
  label: string;
  external?: boolean;
};

type FooterSection = {
  title: string;
  links: FooterLink[];
};

export function Footer() {
  const sections: FooterSection[] = [
    {
      title: "Product",
      links: [
        { href: "#what-is-dx", label: "What is DX" },
        { href: "#generate-anything", label: "Generate" },
        { href: "#token-revolution", label: "Token Revolution" },
        { href: "#pricing", label: "Pricing" },
      ],
    },
    {
      title: "Platform",
      links: [
        { href: "#works-everywhere", label: "Coverage" },
        { href: "#free-ai", label: "Free AI Access" },
        { href: "#built-on-rust", label: "Built on Rust" },
      ],
    },
    {
      title: "Company",
      links: [
        { href: "https://x.com/dxai", label: "X / Twitter", external: true },
        {
          href: "https://www.linkedin.com/company/dx-ai",
          label: "LinkedIn",
          external: true,
        },
        { href: "mailto:hello@dx.ai", label: "Contact", external: true },
      ],
    },
  ];

  return (
    <footer className="bg-background relative overflow-hidden border-t border-border">
      <div className="max-w-[1400px] mx-auto px-4 sm:px-8 py-16 sm:pb-44">
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-14 lg:gap-16">
          <div className="grid grid-cols-2 sm:grid-cols-3 gap-8 sm:gap-12">
            {sections.map((section) => (
              <div key={section.title} className="space-y-3">
                <h3 className="font-sans text-sm text-foreground mb-4">
                  {section.title}
                </h3>
                <div className="space-y-2.5">
                  {section.links.map((item) => (
                    <Link
                      key={item.href}
                      href={item.href}
                      target={item.external ? "_blank" : undefined}
                      rel={item.external ? "noopener noreferrer" : undefined}
                      className="font-sans text-sm text-muted-foreground hover:text-foreground transition-colors block"
                    >
                      {item.label}
                    </Link>
                  ))}
                </div>
              </div>
            ))}
          </div>

          <div className="flex flex-col items-start lg:items-end gap-6 lg:gap-10">
            <p className="font-sans text-base sm:text-xl text-foreground text-left lg:text-right max-w-lg">
              DX unifies AI generation, tool calling, media creation, and deep
              workflow integration into one development experience.
            </p>
            <p className="font-sans text-sm text-muted-foreground text-left lg:text-right max-w-lg">
              Built in Rust. Optimized with RLM and DX Serializer. Designed for
              developers who ship.
            </p>
          </div>
        </div>

        <div className="my-14">
          <div className="h-px w-full border-t border-border" />
        </div>

        <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-4">
          <a
            href="https://dx.openstatus.dev/"
            target="_blank"
            rel="noopener noreferrer"
            className="hidden md:flex items-center gap-2 hover:opacity-80 transition-opacity"
          >
            <span className="font-sans text-sm text-muted-foreground">
              System status:
            </span>
            <span className="font-sans text-sm text-foreground">
              Operational
            </span>
          </a>
          <p className="font-sans text-sm text-muted-foreground">
            © {new Date().getFullYear()} DX Labs AB. All rights reserved.
          </p>
        </div>
      </div>

      <div className="absolute bottom-0 left-0 sm:left-1/2 sm:-translate-x-1/2 translate-y-[22%] sm:translate-y-[36%] bg-background overflow-hidden pointer-events-none">
        <h1
          className={cn(
            "font-sans text-[190px] sm:text-[470px] leading-none select-none",
            "text-secondary",
            "[WebkitTextStroke:1px_hsl(var(--muted-foreground))]",
            "[textStroke:1px_hsl(var(--muted-foreground))]",
          )}
          style={{
            WebkitTextStroke: "1px hsl(var(--muted-foreground))",
            color: "hsl(var(--secondary))",
          }}
        >
          dx
        </h1>
      </div>
    </footer>
  );
}
