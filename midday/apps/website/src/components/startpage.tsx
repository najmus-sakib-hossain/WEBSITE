"use client";

import { useGSAP } from "@gsap/react";
import gsap from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import { Button } from "@midday/ui/button";
import { motion } from "motion/react";
import Link from "next/link";
import { useRef } from "react";
import { DxAiFace } from "./dx-ai-face";
import { DxVideoShowcases } from "./dx-video-showcases";

gsap.registerPlugin(ScrollTrigger);

const marqueeCompanies = [
  "Arcforge",
  "Neonstack",
  "Byteplane",
  "Graphloom",
  "Shellgrid",
  "Nodecraft",
  "Cloudmesh",
  "Signalbase",
];

const featureCards = [
  {
    title: "Rust-Powered Performance",
    body: "Built from the ground up in Rust. 12ms startup. 45MB RAM baseline. 60fps UI under load.",
  },
  {
    title: "Everything Connected",
    body: "Editor, terminal, assistant, docs, and git state share one execution context.",
  },
  {
    title: "Offline-First",
    body: "Full capability without internet using local models, cached docs, and local workflows.",
  },
  {
    title: "Token-Efficient AI",
    body: "RLM + DX Serializer deliver more context per token and lower cost per workflow.",
  },
  {
    title: "MCP Apps Integration",
    body: "Native MCP app orchestration gives DX direct access to the tools that power your stack.",
  },
  {
    title: "Keyboard-First Workflows",
    body: "Shortcuts, command palette actions, and automations keep your flow uninterrupted.",
  },
];

const comparisonRows = [
  ["Startup Time", "12ms", "1.2s", "3.5s"],
  ["RAM Usage", "45MB", "350MB", "1.2GB"],
  ["Offline AI", "✅", "❌", "❌"],
  ["Token Saving", "✅", "❌", "❌"],
  ["MCP Apps", "✅", "❌", "❌"],
  ["Runtime", "Rust", "Electron", "JVM"],
  ["Automations", "Built-in", "Plugin", "Plugin"],
];

const testimonials = [
  {
    quote: "DX replaced my entire toolchain in one weekend.",
    by: "Staff Engineer, Platform Team",
  },
  {
    quote: "Offline mode is a game changer when I'm traveling.",
    by: "Freelance Developer",
  },
  {
    quote: "MCP integration means our assistant actually knows our stack.",
    by: "CTO, Product Startup",
  },
];

export function StartPage() {
  const scopeRef = useRef<HTMLDivElement>(null);

  useGSAP(
    () => {
      gsap.fromTo(
        ".dx-reveal",
        { opacity: 0, y: 22 },
        {
          opacity: 1,
          y: 0,
          duration: 0.55,
          stagger: 0.07,
          ease: "power2.out",
          scrollTrigger: {
            trigger: scopeRef.current,
            start: "top 80%",
          },
        },
      );

      gsap.fromTo(
        ".dx-bar",
        { scaleX: 0.08 },
        {
          scaleX: 1,
          duration: 0.9,
          ease: "power2.out",
          transformOrigin: "left center",
          stagger: 0.08,
          scrollTrigger: {
            trigger: ".dx-bench-wrap",
            start: "top 82%",
          },
        },
      );
    },
    { scope: scopeRef },
  );

  return (
    <div ref={scopeRef} className="min-h-screen bg-background pb-20">
      <section className="pt-32 sm:pt-36">
        <div className="max-w-[1150px] mx-auto px-4 sm:px-8">
          <div className="dx-reveal text-center">
            <p className="text-xs uppercase tracking-wide text-muted-foreground">Launching February 24, 2026</p>
            <div className="mt-6">
              <DxAiFace />
            </div>
            <p className="mt-6 text-base sm:text-lg text-muted-foreground">Hi. I&apos;m DX.</p>
            <h1 className="mt-3 font-serif text-4xl sm:text-5xl lg:text-6xl leading-tight text-foreground">
              The Developer Experience You Actually Deserve.
            </h1>
            <p className="mt-5 max-w-3xl mx-auto text-muted-foreground">
              Built with Rust. Blazing fast. Offline-ready. Everything connected. DX unifies AI generation,
              tool calling, media workflows, and automations in one integrated runtime.
            </p>
            <div className="mt-8 flex flex-col sm:flex-row justify-center gap-3">
              <Button asChild className="btn-inverse h-11 px-6">
                <Link href="/download">Get Started Free</Link>
              </Button>
              <Button asChild variant="outline" className="h-11 px-6">
                <a href="#showcases">Watch Demo</a>
              </Button>
            </div>
          </div>

          <div className="dx-reveal mt-12 border border-border overflow-hidden">
            <motion.div
              className="flex min-w-max"
              animate={{ x: [0, -600] }}
              transition={{ duration: 18, repeat: Number.POSITIVE_INFINITY, ease: "linear" }}
            >
              {[...marqueeCompanies, ...marqueeCompanies].map((name, index) => (
                <div
                  key={`${name}-${index}`}
                  className="px-8 py-3 border-r border-border text-sm text-muted-foreground whitespace-nowrap"
                >
                  Trusted by {name}
                </div>
              ))}
            </motion.div>
          </div>
        </div>
      </section>

      <section id="showcases" className="dx-reveal pt-14">
        <div className="max-w-[1150px] mx-auto px-4 sm:px-8">
          <DxVideoShowcases />
        </div>
      </section>

      <section className="dx-reveal pt-14">
        <div className="max-w-[1150px] mx-auto px-4 sm:px-8">
          <div className="border border-border p-5 sm:p-7">
            <h2 className="font-serif text-3xl text-foreground">Why DX?</h2>
            <div className="mt-6 grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
              {featureCards.map((item) => (
                <div key={item.title} className="border border-border p-4">
                  <h3 className="text-foreground text-base">{item.title}</h3>
                  <p className="mt-2 text-sm text-muted-foreground">{item.body}</p>
                </div>
              ))}
            </div>
          </div>
        </div>
      </section>

      <section className="dx-reveal pt-14">
        <div className="max-w-[1150px] mx-auto px-4 sm:px-8">
          <div className="border border-border p-5 sm:p-7 dx-bench-wrap">
            <h2 className="font-serif text-3xl text-foreground">Built With Rust. Built To Fly.</h2>
            <p className="mt-3 text-muted-foreground max-w-3xl">
              DX is engineered in Rust for memory safety and high throughput. It keeps startup instant, UI responsive,
              and workflows stable under heavy project loads.
            </p>

            <div className="mt-8 grid grid-cols-1 md:grid-cols-2 gap-6">
              <div className="space-y-4">
                {[
                  ["DX", "12ms startup"],
                  ["VS Code", "1.2s startup"],
                  ["JetBrains", "3.5s startup"],
                ].map(([label, value]) => (
                  <div key={label}>
                    <div className="flex items-center justify-between text-sm">
                      <span className="text-foreground">{label}</span>
                      <span className="text-muted-foreground">{value}</span>
                    </div>
                    <div className="mt-2 h-2 bg-secondary/40 border border-border overflow-hidden">
                      <div className="dx-bar h-full bg-foreground" />
                    </div>
                  </div>
                ))}
              </div>

              <div className="border border-border p-4">
                <p className="text-sm text-muted-foreground">Deep Dive</p>
                <p className="mt-2 text-foreground">
                  Zero garbage collection pauses. Low memory pressure. Native execution path for editor and assistant workflows.
                </p>
                <p className="mt-4 text-sm text-muted-foreground">
                  12ms startup · 45MB RAM baseline · 60fps UI under heavy file and tool workloads.
                </p>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="dx-reveal pt-14">
        <div className="max-w-[1150px] mx-auto px-4 sm:px-8">
          <div className="border border-border p-5 sm:p-7">
            <h2 className="font-serif text-3xl text-foreground">Developer Testimonials</h2>
            <div className="mt-6 grid grid-cols-1 md:grid-cols-3 gap-4">
              {testimonials.map((item) => (
                <div key={item.quote} className="border border-border p-4">
                  <p className="text-foreground">“{item.quote}”</p>
                  <p className="mt-3 text-xs text-muted-foreground">— {item.by}</p>
                </div>
              ))}
            </div>
          </div>
        </div>
      </section>

      <section className="dx-reveal pt-14">
        <div className="max-w-[1150px] mx-auto px-4 sm:px-8">
          <div className="border border-border p-5 sm:p-7 overflow-x-auto">
            <h2 className="font-serif text-3xl text-foreground">Comparison Table</h2>
            <table className="w-full min-w-[680px] mt-5 text-sm">
              <thead>
                <tr className="border-b border-border text-muted-foreground">
                  <th className="text-left py-2">Feature</th>
                  <th className="text-left py-2">DX</th>
                  <th className="text-left py-2">VS Code</th>
                  <th className="text-left py-2">JetBrains</th>
                </tr>
              </thead>
              <tbody>
                {comparisonRows.map((row) => (
                  <tr key={row[0]} className="border-b border-border">
                    <td className="py-2 text-foreground">{row[0]}</td>
                    <td className="py-2 text-foreground">{row[1]}</td>
                    <td className="py-2 text-muted-foreground">{row[2]}</td>
                    <td className="py-2 text-muted-foreground">{row[3]}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </section>

      <section className="dx-reveal pt-14">
        <div className="max-w-[1150px] mx-auto px-4 sm:px-8">
          <div className="border border-border p-6 sm:p-10 text-center">
            <p className="text-xs uppercase tracking-wide text-muted-foreground">Interactive Demo</p>
            <h3 className="mt-3 font-serif text-3xl sm:text-4xl text-foreground">Try the DX workflow playground.</h3>
            <p className="mt-4 text-muted-foreground max-w-2xl mx-auto">
              See connected generation, MCP actions, automations, and offline mode behavior in one guided demo.
            </p>
            <div className="mt-7 flex flex-wrap justify-center gap-3">
              <Button asChild className="btn-inverse h-11 px-8">
                <Link href="/assistant">Open Assistant</Link>
              </Button>
              <Button asChild variant="outline" className="h-11 px-8">
                <Link href="/integrations">Explore Integrations</Link>
              </Button>
            </div>
          </div>
        </div>
      </section>

      <section id="waitlist" className="dx-reveal pt-14">
        <div className="max-w-[1150px] mx-auto px-4 sm:px-8">
          <motion.div
            className="border border-border p-6 sm:p-10 text-center"
            initial={{ opacity: 0, y: 14 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true, amount: 0.2 }}
            transition={{ duration: 0.35 }}
          >
            <p className="text-muted-foreground text-sm uppercase tracking-wide">Final CTA</p>
            <h3 className="mt-3 font-serif text-3xl sm:text-4xl text-foreground">Build faster. Ship smarter. Stay in flow.</h3>
            <p className="mt-4 text-muted-foreground max-w-2xl mx-auto">
              DX launch is live. Download now and start with the full connected developer experience.
            </p>
            <div className="mt-7 flex justify-center">
              <Button asChild className="btn-inverse h-11 px-8">
                <Link href="/download">Download DX</Link>
              </Button>
            </div>
          </motion.div>
        </div>
      </section>
    </div>
  );
}
