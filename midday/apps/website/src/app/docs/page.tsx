import type { Metadata } from "next";
import { baseUrl } from "@/app/sitemap";
import { DxVideoCarouselSections } from "@/components/dx-video-carousel-sections";

const title = "DX Documentation";
const description =
  "Technical documentation for DX architecture, connected workflows, token-saving systems, offline capability, and integration runtime behavior.";

export const metadata: Metadata = {
  title,
  description,
  openGraph: {
    title,
    description,
    type: "website",
    url: `${baseUrl}/docs`,
  },
  twitter: {
    card: "summary_large_image",
    title,
    description,
  },
  alternates: {
    canonical: `${baseUrl}/docs`,
  },
};

export default function DocsPage() {
  return (
    <DxVideoCarouselSections
      pageTitle="DX Documentation"
      pageDescription="Documentation in DX is implementation-first: architecture, token pipeline internals, connected execution, and reproducible workflow patterns."
    />
  );
}
