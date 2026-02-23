"use client";

import { Button } from "@midday/ui/button";
import { motion } from "motion/react";
import type { ReactNode } from "react";

const generationCategories = [
  {
    category: "Code",
    capability: "Any language, any framework, full-project scaffolding",
  },
  {
    category: "Charts & Data",
    capability: "Visualizations, dashboards, and analysis",
  },
  {
    category: "Deep Research",
    capability: "Multi-step reasoning, deep dives, synthesis",
  },
  {
    category: "Tool Calling",
    capability: "Full support for MCP, ACP, and A2A protocols",
  },
  {
    category: "Video",
    capability: "AI video generation and editing",
  },
  {
    category: "3D",
    capability: "3D asset and scene generation",
  },
  {
    category: "Audio & Music",
    capability: "Sound design, composition, and voice synthesis",
  },
  {
    category: "Conversation",
    capability: "Real-time voice interaction",
  },
];

const platforms = [
  ["macOS", "Native Desktop App", "✅ Launch"],
  ["Windows", "Native Desktop App", "✅ Launch"],
  ["Linux", "Native Desktop App", "✅ Launch"],
  ["Android", "Mobile App", "✅ Launch"],
  ["iOS", "Mobile App", "✅ Launch"],
  ["Browser", "Extension", "✅ Launch"],
  ["IDEs/Editors", "Extensions", "✅ Launch"],
  ["Video Editors", "Plugins", "✅ Launch"],
  ["Image Editors", "Plugins", "✅ Launch"],
];

const comparisons = [
  ["Core Language", "Rust + GPUI", "Node.js / Electron"],
  ["Token Efficiency", "80–90% savings (RLM)", "No RLM implementation"],
  [
    "Serialization",
    "DX Serializer (70–90% savings)",
    "Raw JSON payloads",
  ],
  ["Offline Support", "Unlimited, free", "Internet + paid tiers"],
  ["AI Provider Support", "Any provider", "Locked to 1–3 providers"],
  [
    "Media Generation",
    "Code, video, 3D, audio, music",
    "Mostly code only",
  ],
  [
    "Platform Coverage",
    "5 OS + extensions everywhere",
    "1–2 platforms, limited plugins",
  ],
];

const fadeIn = {
  initial: { opacity: 0, y: 18 },
  whileInView: { opacity: 1, y: 0 },
  viewport: { once: true, amount: 0.2 },
  transition: { duration: 0.45 },
};

function Section({
  id,
  title,
  subtitle,
  children,
}: {
  id: string;
  title: string;
  subtitle?: string;
  children: ReactNode;
}) {
  return (
    <motion.section
      id={id}
      className="py-16 sm:py-20 border-t border-border"
      {...fadeIn}
    >
      <div className="max-w-[1100px] mx-auto px-4 sm:px-8">
        <h2 className="font-serif text-2xl sm:text-3xl text-foreground">{title}</h2>
        {subtitle ? (
          <p className="mt-3 text-base text-muted-foreground max-w-3xl">{subtitle}</p>
        ) : null}
        <div className="mt-8">{children}</div>
      </div>
    </motion.section>
  );
}

export function StartPage() {
  return (
    <div className="min-h-screen bg-background">
      <section className="pt-36 pb-16 sm:pt-40 sm:pb-20">
        <div className="max-w-[1100px] mx-auto px-4 sm:px-8">
          <motion.div {...fadeIn}>
            <p className="text-xs sm:text-sm tracking-wide text-muted-foreground uppercase">
              Launching February 24, 2026
            </p>
            <h1 className="mt-4 font-serif text-4xl sm:text-5xl lg:text-6xl leading-tight text-foreground max-w-4xl">
              Enhance Your Development Experience.
            </h1>
            <p className="mt-6 text-base sm:text-lg text-muted-foreground max-w-3xl">
              DX is not a chatbot. Not just an AI agent. Not another wrapper around an LLM.
              DX is a unified development experience platform where code generation, research,
              tool orchestration, video, 3D, and audio are connected by one purpose: helping you build faster.
            </p>
            <div className="mt-8 flex flex-col sm:flex-row gap-3">
              <Button asChild className="btn-inverse h-11 px-6">
                <a href="#waitlist">Join Early Access</a>
              </Button>
              <Button asChild variant="outline" className="h-11 px-6">
                <a href="#what-is-dx">Explore DX</a>
              </Button>
            </div>
          </motion.div>

          <motion.div
            className="mt-12 grid grid-cols-1 md:grid-cols-3 gap-4"
            initial={{ opacity: 0 }}
            whileInView={{ opacity: 1 }}
            viewport={{ once: true, amount: 0.2 }}
            transition={{ duration: 0.5, delay: 0.1 }}
          >
            {[
              "Built on Rust + GPUI",
              "Any AI provider + offline local models",
              "80–90% token savings on large operations",
            ].map((item) => (
              <div key={item} className="border border-border bg-background p-4 text-sm text-foreground">
                {item}
              </div>
            ))}
          </motion.div>
        </div>
      </section>

      <Section
        id="what-is-dx"
        title="What Is DX?"
        subtitle="DX is a unified development experience platform. There are no arbitrary category boundaries — everything is one connected system."
      >
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm sm:text-base">
          <div className="border border-border p-5 text-muted-foreground">
            AI generation, tool calling, media creation, and workflow integration are not separate products.
            They are facets of one cohesive experience.
          </div>
          <div className="border border-border p-5 text-muted-foreground">
            You can generate code, analyze data, run deep research, and produce media with one consistent workflow,
            one context, and one mental model.
          </div>
        </div>
      </Section>

      <Section
        id="built-on-rust"
        title="Built on Rust. Not Node.js. Not Electron."
        subtitle="DX is engineered in Rust for performance, efficiency, and native-grade responsiveness across platforms."
      >
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="border border-border p-5">
            <p className="text-sm text-muted-foreground">Speed</p>
            <p className="mt-2 text-foreground">Near-native performance on every operation.</p>
          </div>
          <div className="border border-border p-5">
            <p className="text-sm text-muted-foreground">Efficiency</p>
            <p className="mt-2 text-foreground">Scales from low-end hardware to workstation-class machines.</p>
          </div>
          <div className="border border-border p-5">
            <p className="text-sm text-muted-foreground">Desktop UI</p>
            <p className="mt-2 text-foreground">GPUI-powered rendering for a fast, responsive native experience.</p>
          </div>
        </div>
      </Section>

      <Section
        id="generate-anything"
        title="Generate Literally Anything"
        subtitle="If you can name it, DX can generate it."
      >
        <div className="overflow-x-auto border border-border">
          <table className="w-full text-sm">
            <thead className="bg-secondary/40">
              <tr>
                <th className="text-left p-3 text-foreground font-medium">Category</th>
                <th className="text-left p-3 text-foreground font-medium">Capabilities</th>
              </tr>
            </thead>
            <tbody>
              {generationCategories.map((row) => (
                <tr key={row.category} className="border-t border-border">
                  <td className="p-3 text-foreground">{row.category}</td>
                  <td className="p-3 text-muted-foreground">{row.capability}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Section>

      <Section
        id="token-revolution"
        title="Token Revolution"
        subtitle="RLM + DX Serializer + micro-optimizations across the full pipeline."
      >
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="border border-border p-5">
            <p className="text-foreground">RLM</p>
            <p className="mt-2 text-muted-foreground text-sm">
              Saves 80–90% tokens on large file operations by minimizing reference length in context flows.
            </p>
          </div>
          <div className="border border-border p-5">
            <p className="text-foreground">DX Serializer</p>
            <p className="mt-2 text-muted-foreground text-sm">
              Saves 70–90% tokens on tool calls by replacing bloated JSON transport.
            </p>
          </div>
          <div className="border border-border p-5">
            <p className="text-foreground">Compound Savings</p>
            <p className="mt-2 text-muted-foreground text-sm">
              Savings stack across operations, making complex agent workflows economically viable.
            </p>
          </div>
        </div>
      </Section>

      <Section
        id="works-everywhere"
        title="Works Everywhere"
        subtitle="Native apps and extensions across the full development and creative workflow."
      >
        <div className="overflow-x-auto border border-border">
          <table className="w-full text-sm">
            <thead className="bg-secondary/40">
              <tr>
                <th className="text-left p-3 text-foreground font-medium">Platform</th>
                <th className="text-left p-3 text-foreground font-medium">App Type</th>
                <th className="text-left p-3 text-foreground font-medium">Status</th>
              </tr>
            </thead>
            <tbody>
              {platforms.map(([platform, appType, status]) => (
                <tr key={platform} className="border-t border-border">
                  <td className="p-3 text-foreground">{platform}</td>
                  <td className="p-3 text-muted-foreground">{appType}</td>
                  <td className="p-3 text-foreground">{status}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Section>

      <Section
        id="free-ai"
        title="Free AI Access — Any Provider, Even Offline"
        subtitle="Own your workflow. No vendor lock-in."
      >
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="border border-border p-5 text-muted-foreground">
            <p className="text-foreground">Online</p>
            <p className="mt-2 text-sm">Connect to OpenAI, Anthropic, Google, Mistral, open-source, and self-hosted endpoints.</p>
          </div>
          <div className="border border-border p-5 text-muted-foreground">
            <p className="text-foreground">Offline</p>
            <p className="mt-2 text-sm">Run capable local models without internet and without token limits.</p>
          </div>
          <div className="border border-border p-5 text-muted-foreground">
            <p className="text-foreground">Hybrid</p>
            <p className="mt-2 text-sm">Switch seamlessly between cloud and local models based on runtime conditions.</p>
          </div>
        </div>
      </Section>

      <Section
        id="competitive"
        title="Competitive Positioning"
        subtitle="Technical differences that matter in production workflows."
      >
        <div className="overflow-x-auto border border-border">
          <table className="w-full text-sm">
            <thead className="bg-secondary/40">
              <tr>
                <th className="text-left p-3 text-foreground font-medium">Feature</th>
                <th className="text-left p-3 text-foreground font-medium">DX</th>
                <th className="text-left p-3 text-foreground font-medium">Competitors</th>
              </tr>
            </thead>
            <tbody>
              {comparisons.map(([feature, dx, competitors]) => (
                <tr key={feature} className="border-t border-border">
                  <td className="p-3 text-foreground">{feature}</td>
                  <td className="p-3 text-foreground">{dx}</td>
                  <td className="p-3 text-muted-foreground">{competitors}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Section>

      <Section id="pricing" title="Pricing" subtitle="Generous free access. Transparent paid tiers.">
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          {[
            {
              title: "Free",
              price: "$0",
              details: "Start with core generation, local workflows, and integrations.",
            },
            {
              title: "Pro",
              price: "$19/mo",
              details: "Higher limits, advanced orchestration, and team-ready workflows.",
            },
            {
              title: "Scale",
              price: "Custom",
              details: "Enterprise controls, private deployment options, and support SLAs.",
            },
          ].map((plan) => (
            <div key={plan.title} className="border border-border p-5">
              <p className="text-sm text-muted-foreground">{plan.title}</p>
              <p className="mt-2 text-2xl text-foreground">{plan.price}</p>
              <p className="mt-3 text-sm text-muted-foreground">{plan.details}</p>
            </div>
          ))}
        </div>
      </Section>

      <section id="waitlist" className="py-16 sm:py-24 border-t border-border">
        <div className="max-w-[1100px] mx-auto px-4 sm:px-8">
          <motion.div
            className="border border-border p-6 sm:p-10 text-center"
            {...fadeIn}
          >
            <p className="text-muted-foreground text-sm uppercase tracking-wide">Early Access</p>
            <h3 className="mt-3 font-serif text-3xl sm:text-4xl text-foreground">Be first on DX launch day.</h3>
            <p className="mt-4 text-muted-foreground max-w-2xl mx-auto">
              Launching February 24, 2026. Join the waitlist for priority access, release notes, and first-week benchmarks.
            </p>
            <div className="mt-7 flex justify-center">
              <Button asChild className="btn-inverse h-11 px-8">
                <a href="mailto:hello@dx.ai?subject=DX%20Early%20Access">Join Waitlist</a>
              </Button>
            </div>
          </motion.div>
        </div>
      </section>
    </div>
  );
}
