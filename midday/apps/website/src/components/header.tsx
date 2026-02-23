"use client";

import { Button } from "@midday/ui/button";
import { cn } from "@midday/ui/cn";
import { Icons } from "@midday/ui/icons";
import { motion } from "motion/react";
import Link from "next/link";
import { useState } from "react";

interface HeaderProps {
  transparent?: boolean;
  hideMenuItems?: boolean;
}

const navigation = [
  { href: "#what-is-dx", label: "What is DX" },
  { href: "#built-on-rust", label: "Built on Rust" },
  { href: "#generate-anything", label: "Generate" },
  { href: "#token-revolution", label: "Token" },
  { href: "#works-everywhere", label: "Coverage" },
  { href: "#pricing", label: "Pricing" },
];

export function Header({ transparent = false, hideMenuItems = false }: HeaderProps) {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <header className="fixed top-0 left-0 right-0 z-50">
      <div
        className={cn(
          "py-3 xl:py-4 px-4 sm:px-4 md:px-4 lg:px-4 xl:px-6 2xl:px-8",
          "flex items-center justify-between border-b border-border",
          transparent ? "bg-background/60 backdrop-blur-md" : "bg-background/80 backdrop-blur-md",
        )}
      >
        <Link
          href="/"
          className="flex items-center gap-2 hover:opacity-80 transition-opacity"
          aria-label="DX - Go to homepage"
          onClick={() => setIsOpen(false)}
        >
          <div className="w-6 h-6">
            <Icons.LogoSmall className="w-full h-full text-foreground" />
          </div>
          <span className="font-sans text-base text-foreground">dx</span>
        </Link>

        {!hideMenuItems ? (
          <nav className="hidden xl:flex items-center gap-6">
            {navigation.map((item) => (
              <a
                key={item.href}
                href={item.href}
                className="text-sm text-muted-foreground hover:text-foreground transition-colors"
              >
                {item.label}
              </a>
            ))}
          </nav>
        ) : null}

        <div className="hidden xl:flex items-center gap-2">
          <Button asChild variant="outline" className="h-9 px-4">
            <a href="#what-is-dx">Read Docs</a>
          </Button>
          <Button asChild className="btn-inverse h-9 px-4">
            <a href="#waitlist">Join Waitlist</a>
          </Button>
        </div>

        <button
          type="button"
          className="xl:hidden p-2 text-muted-foreground hover:text-foreground"
          aria-label="Toggle menu"
          onClick={() => setIsOpen((prev) => !prev)}
        >
          {isOpen ? <Icons.Close /> : <Icons.Menu />}
        </button>
      </div>

      {isOpen ? (
        <motion.div
          initial={{ opacity: 0, y: -8 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -8 }}
          className="xl:hidden border-b border-border bg-background/95 backdrop-blur-md"
        >
          <div className="px-4 py-4 flex flex-col gap-2">
            {navigation.map((item) => (
              <a
                key={item.href}
                href={item.href}
                className="text-sm text-muted-foreground hover:text-foreground px-2 py-2"
                onClick={() => setIsOpen(false)}
              >
                {item.label}
              </a>
            ))}
            <div className="pt-2 grid grid-cols-2 gap-2">
              <Button asChild variant="outline" className="h-9">
                <a href="#what-is-dx" onClick={() => setIsOpen(false)}>
                  Read Docs
                </a>
              </Button>
              <Button asChild className="btn-inverse h-9">
                <a href="#waitlist" onClick={() => setIsOpen(false)}>
                  Join Waitlist
                </a>
              </Button>
            </div>
          </div>
        </motion.div>
      ) : null}
    </header>
  );
}
