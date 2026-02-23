"use client";

import { Button } from "@midday/ui/button";
import { Icons } from "@midday/ui/icons";
import dynamic from "next/dynamic";
import Image from "next/image";
import { useEffect, useRef, useState } from "react";

// Dynamic imports for below-the-fold sections (still SSR for SEO)
const FeaturesGridSection = dynamic(() =>
  import("./sections/features-grid-section").then((m) => m.FeaturesGridSection),
);
const TimeSavingsSection = dynamic(() =>
  import("./sections/time-savings-section").then((m) => m.TimeSavingsSection),
);
const WeeklyAudioSection = dynamic(() =>
  import("./sections/weekly-audio-section").then((m) => m.WeeklyAudioSection),
);
const PreAccountingSection = dynamic(() =>
  import("./sections/pre-accounting-section").then(
    (m) => m.PreAccountingSection,
  ),
);
const TestimonialsSection = dynamic(
  () =>
    import("./sections/testimonials-section").then(
      (m) => m.TestimonialsSection,
    ),
  { ssr: false },
);
const IntegrationsSection = dynamic(() =>
  import("./sections/integrations-section").then((m) => m.IntegrationsSection),
);
const PricingSection = dynamic(() =>
  import("./sections/pricing-section").then((m) => m.PricingSection),
);

const features = [
  {
    title: "Generate Literally Anything",
    subtitle:
      "Code, Charts, Deep Research, Tool Calling, Video, 3D, Audio & Music, Conversation.",
    mobileSubtitle: "Code, Video, 3D, Audio, and more.",
    illustration: "animation",
  },
  {
    title: "Built on Rust",
    subtitle:
      "Engineered from the ground up in Rust. Near-native performance on every operation.",
    mobileSubtitle:
      "Engineered in Rust for near-native performance.",
    illustration: "animation",
  },
  {
    title: "Revolutionary Token Savings",
    subtitle:
      "RLM techniques save 80–90% of tokens on large files. DX Serializer saves 70–90% on tool calls.",
    mobileSubtitle:
      "Save 80-90% of tokens with RLM and DX Serializer.",
    illustration: "animation",
  },
  {
    title: "Free AI Access",
    subtitle:
      "Connect to any major or minor LLM provider, or run capable local models offline, with no token limits.",
    mobileSubtitle: "Any provider, even offline.",
    illustration: "animation",
  },
  {
    title: "Extensions Everywhere",
    subtitle:
      "Works in any browser, IDE, video editor, and image design tool.",
    mobileSubtitle: "Integrates into the tools you already use.",
    illustration: "animation",
  },
];

const videos = [
  {
    id: "overview",
    title: "Overview",
    subtitle:
      "See how DX enhances your development experience.",
    url: "https://cdn.midday.ai/videos/login-video.mp4",
  },
  {
    id: "assistant",
    title: "Assistant",
    subtitle:
      "Ask questions and get clear answers based on your codebase.",
    url: "https://cdn.midday.ai/videos/login-video.mp4",
  },
  {
    id: "insights",
    title: "Insights",
    subtitle:
      "Understand how your project evolves with live widgets and summaries.",
    url: "https://cdn.midday.ai/videos/login-video.mp4",
  },
];

export function StartPage() {
  const [activeFeature, setActiveFeature] = useState(0);
  const [isVideoLoaded, setIsVideoLoaded] = useState(false);
  const [isPosterLoaded, setIsPosterLoaded] = useState(false);
  const [isDashboardLightLoaded, setIsDashboardLightLoaded] = useState(false);
  const [isDashboardDarkLoaded, setIsDashboardDarkLoaded] = useState(false);
  const [isVideoModalOpen, setIsVideoModalOpen] = useState(false);
  const [activeVideoId, setActiveVideoId] = useState("overview");
  const [videoProgress, setVideoProgress] = useState(0);

  const videoContainerRef = useRef(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const modalVideoRef = useRef<HTMLVideoElement>(null);
  const videoTagsScrollRef = useRef<HTMLDivElement>(null);
  const styleSheetRef = useRef<HTMLStyleElement | null>(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;

    const handleLoad = () => setIsVideoLoaded(true);

    if (video.readyState >= 3) {
      setIsVideoLoaded(true);
    }

    video.addEventListener("canplay", handleLoad);
    video.addEventListener("loadeddata", handleLoad);

    return () => {
      video.removeEventListener("canplay", handleLoad);
      video.removeEventListener("loadeddata", handleLoad);
    };
  }, []);

  useEffect(() => {
    const video = modalVideoRef.current;
    if (!video || !isVideoModalOpen) return;

    const activeVideo = videos.find((v) => v.id === activeVideoId);
    if (activeVideo) {
      if (video.src !== activeVideo.url) {
        video.src = activeVideo.url;
        video.load();
        setVideoProgress(0);
      }
      const handleCanPlay = () => {
        const playPromise = video.play();
        if (playPromise !== undefined) {
          playPromise.catch(() => {});
        }
      };
      video.addEventListener("canplay", handleCanPlay);
      if (video.readyState >= 3) {
        handleCanPlay();
      }
      return () => {
        video.removeEventListener("canplay", handleCanPlay);
      };
    }
  }, [activeVideoId, isVideoModalOpen]);

  useEffect(() => {
    const video = modalVideoRef.current;
    if (!video || !isVideoModalOpen) return;

    const updateProgress = () => {
      if (video.duration) {
        const progress = (video.currentTime / video.duration) * 100;
        setVideoProgress(progress);
      }
    };

    const handleTimeUpdate = () => updateProgress();
    const handleLoadedMetadata = () => {
      setVideoProgress(0);
      updateProgress();
    };

    video.addEventListener("timeupdate", handleTimeUpdate);
    video.addEventListener("loadedmetadata", handleLoadedMetadata);

    return () => {
      video.removeEventListener("timeupdate", handleTimeUpdate);
      video.removeEventListener("loadedmetadata", handleLoadedMetadata);
    };
  }, [activeVideoId, isVideoModalOpen]);

  useEffect(() => {
    if (!isVideoModalOpen) return;

    const style = document.createElement("style");
    style.textContent = `
      @keyframes fadeIn {
        from { opacity: 0; }
        to { opacity: 1; }
      }
      video::-webkit-media-controls-timeline,
      video::-webkit-media-controls-current-time-display,
      video::-webkit-media-controls-time-remaining-display,
      video::-webkit-media-controls-timeline-container,
      video::-webkit-media-controls-panel {
        display: none !important;
      }
      video {
        width: 100% !important;
        height: 100% !important;
        object-fit: cover !important;
      }
    `;
    document.head.appendChild(style);
    styleSheetRef.current = style;

    return () => {
      if (styleSheetRef.current) {
        document.head.removeChild(styleSheetRef.current);
        styleSheetRef.current = null;
      }
    };
  }, [isVideoModalOpen]);

  return (
    <div className="min-h-screen">
      {/* Hero Section */}
      <div className="bg-background relative min-h-screen overflow-visible lg:overflow-hidden">
        <div className="flex flex-col min-h-screen relative pt-32 pb-12 sm:py-32 md:pt-24 lg:pt-0 overflow-hidden">
          <div className="flex-1 lg:flex-none flex flex-col justify-center md:justify-start md:pt-16 lg:pt-56 items-center lg:items-stretch space-y-8 lg:space-y-0 z-20 px-3 sm:px-4 lg:px-0 lg:max-w-[1400px] lg:mx-auto lg:w-full lg:mb-12 xl:mb-12 2xl:mb-12 3xl:mb-16">
            <div className="flex flex-col lg:flex-row lg:justify-between lg:items-end w-full space-y-8 lg:space-y-0">
              <div className="space-y-4 lg:space-y-3 text-center lg:text-left max-w-xl mx-auto lg:mx-0 px-2 lg:px-0">
                <h1 className="font-serif text-3xl sm:text-3xl md:text-3xl lg:text-3xl xl:text-3xl 2xl:text-3xl 3xl:text-4xl leading-tight lg:leading-tight xl:leading-[1.3]">
                  <span className="text-foreground">
                    Enhance Your Development Experience.
                  </span>
                </h1>

                <p className="text-muted-foreground text-base leading-normal font-sans max-w-md lg:max-w-none text-center mx-auto lg:text-left lg:mx-0">
                  DX is a unified development experience platform — a single, blazing-fast tool that connects AI generation, tool calling, media creation, and deep workflow integration under one roof.
                </p>
              </div>

              <div className="space-y-4 text-center lg:text-right w-full lg:w-auto lg:flex lg:flex-col lg:items-end">
                <div className="flex flex-col gap-3 w-full max-w-md mx-auto lg:mx-0 lg:w-auto">
                  <Button
                    asChild
                    className="w-full lg:w-auto btn-inverse h-11 px-5 lg:px-4 transition-colors"
                  >
                    <a href="#">
                      <span className="text-inherit text-sm">
                        Join the Waitlist
                      </span>
                    </a>
                  </Button>
                </div>

                <p className="text-muted-foreground text-xs font-sans">
                  <span className="lg:hidden">
                    Launching February 24, 2026
                  </span>
                  <span className="hidden lg:inline">
                    Launching February 24, 2026
                  </span>
                </p>
              </div>
            </div>
          </div>

          {/* Video section */}
          <div
            className="mt-8 mb-8 md:mt-12 lg:mt-0 lg:mb-4 3xl:mb-20 overflow-visible lg:w-full"
            ref={videoContainerRef}
          >
            <div className="relative overflow-hidden">
              <div
                className={`absolute inset-0 w-full h-full transition-all duration-1000 ease-in-out z-[1] ${
                  isVideoLoaded
                    ? "opacity-0 pointer-events-none"
                    : "opacity-100"
                }`}
                style={{
                  filter: isVideoLoaded ? "blur(0px)" : "blur(1px)",
                }}
              >
                <Image
                  src="https://cdn.midday.ai/video-poster-v2.jpg"
                  alt="DX dashboard preview"
                  fill
                  fetchPriority="high"
                  quality={50}
                  sizes="100vw"
                  className="object-cover transition-all duration-1000 ease-in-out"
                  style={{
                    filter: isPosterLoaded ? "blur(0px)" : "blur(12px)",
                    transform: isPosterLoaded ? "scale(1)" : "scale(1.05)",
                  }}
                  priority
                  onLoad={() => setIsPosterLoaded(true)}
                />
              </div>

              <video
                ref={videoRef}
                className={`w-full h-[420px] sm:h-[520px] md:h-[600px] lg:h-[800px] xl:h-[900px] 3xl:h-[1000px] object-cover transition-opacity duration-1000 ease-in-out ${
                  isVideoLoaded ? "opacity-100" : "opacity-0"
                }`}
                autoPlay
                loop
                muted
                playsInline
                preload="none"
              >
                <source
                  src="https://cdn.midday.ai/videos/login-video.mp4"
                  type="video/mp4"
                />
              </video>

              <div className="absolute inset-0 flex items-center justify-center p-0 lg:p-4 z-[2]">
                <div className="relative lg:static scale-[0.95] md:scale-100 lg:scale-100 lg:h-full lg:flex lg:flex-col lg:items-center lg:justify-center">
                  <Image
                    src="/images/dashboard-light.svg"
                    alt="Dashboard illustration"
                    width={1600}
                    height={1200}
                    className="w-full h-auto md:!scale-[0.85] lg:!scale-100 lg:object-contain lg:max-w-[85%] 2xl:max-w-[75%] dark:hidden lg:[transform:rotate(-2deg)_skewY(1deg)] lg:[filter:drop-shadow(0_30px_60px_rgba(0,0,0,0.6))] transition-all duration-700 ease-out"
                    style={{
                      filter: isDashboardLightLoaded
                        ? "blur(0px) drop-shadow(0 30px 60px rgba(0,0,0,0.6))"
                        : "blur(20px)",
                      transform: isDashboardLightLoaded
                        ? "scale(1)"
                        : "scale(1.02)",
                    }}
                    priority
                    fetchPriority="high"
                    onLoad={() => setIsDashboardLightLoaded(true)}
                  />
                  <Image
                    src="/images/dashboard-dark.svg"
                    alt="Dashboard illustration"
                    width={1600}
                    height={1200}
                    className="w-full h-auto md:!scale-[0.85] lg:!scale-100 lg:object-contain lg:max-w-[85%] 2xl:max-w-[75%] hidden dark:block lg:[transform:rotate(-2deg)_skewY(1deg)] lg:[filter:drop-shadow(0_30px_60px_rgba(0,0,0,0.6))] transition-all duration-700 ease-out"
                    style={{
                      filter: isDashboardDarkLoaded
                        ? "blur(0px) drop-shadow(0 30px 60px rgba(0,0,0,0.6))"
                        : "blur(20px)",
                      transform: isDashboardDarkLoaded
                        ? "scale(1)"
                        : "scale(1.02)",
                    }}
                    priority
                    fetchPriority="high"
                    onLoad={() => setIsDashboardDarkLoaded(true)}
                  />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Features Section */}
      <div className="py-24 sm:py-32 bg-background">
        <div className="mx-auto max-w-7xl px-6 lg:px-8">
          <div className="mx-auto max-w-2xl lg:text-center">
            <h2 className="text-base font-semibold leading-7 text-primary">What is DX?</h2>
            <p className="mt-2 text-3xl font-bold tracking-tight text-foreground sm:text-4xl">
              Not a chatbot. Not just an AI agent.
            </p>
            <p className="mt-6 text-lg leading-8 text-muted-foreground">
              DX is a unified development experience platform. Every feature exists for one purpose: to enhance how developers and creators build.
            </p>
          </div>
          <div className="mx-auto mt-16 max-w-2xl sm:mt-20 lg:mt-24 lg:max-w-none">
            <dl className="grid max-w-xl grid-cols-1 gap-x-8 gap-y-16 lg:max-w-none lg:grid-cols-3">
              {features.map((feature) => (
                <div key={feature.title} className="flex flex-col">
                  <dt className="flex items-center gap-x-3 text-base font-semibold leading-7 text-foreground">
                    {feature.title}
                  </dt>
                  <dd className="mt-4 flex flex-auto flex-col text-base leading-7 text-muted-foreground">
                    <p className="flex-auto">{feature.subtitle}</p>
                  </dd>
                </div>
              ))}
            </dl>
          </div>
        </div>
      </div>

      {/* Pricing Section */}
      <PricingSection />
    </div>
  );
}
