import type { Metadata } from "next";
import { baseUrl } from "@/app/sitemap";

const title = "About";
const description =
  "About DX. Learn about the team and mission behind a unified development experience platform built for everyone.";

export const metadata: Metadata = {
  title,
  description,
  openGraph: {
    title,
    description,
    type: "website",
    url: `${baseUrl}/about`,
  },
  twitter: {
    card: "summary_large_image",
    title,
    description,
  },
  alternates: {
    canonical: `${baseUrl}/about`,
  },
};

export default function AboutPage() {
  return (
    <div className="pt-32 pb-24">
      <div className="max-w-3xl mx-auto px-4 sm:px-6">
        <h1 className="font-serif text-3xl lg:text-4xl text-foreground mb-4">
          About DX
        </h1>
        <p className="font-sans text-base text-muted-foreground leading-relaxed">
          DX exists to enhance how people build. We are designing one connected
          platform for code, research, automation, and media workflows so teams
          can move faster with less friction.
        </p>
        <p className="font-sans text-base text-muted-foreground leading-relaxed mt-4">
          Our approach is simple: native performance, practical AI, and clear
          workflow control. DX is built to run across devices, work online or
          offline, and stay useful in real production environments.
        </p>
      </div>
    </div>
  );
}
