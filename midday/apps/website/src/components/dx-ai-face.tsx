"use client";

import { motion, useMotionValue, useSpring, useTransform } from "motion/react";
import { useMemo } from "react";

type Particle = {
  id: number;
  x: number;
  y: number;
  size: number;
  delay: number;
};

export function DxAiFace() {
  const mouseX = useMotionValue(0);
  const mouseY = useMotionValue(0);

  const smoothX = useSpring(mouseX, { stiffness: 120, damping: 18 });
  const smoothY = useSpring(mouseY, { stiffness: 120, damping: 18 });

  const rotateX = useTransform(smoothY, [-80, 80], [8, -8]);
  const rotateY = useTransform(smoothX, [-80, 80], [-8, 8]);

  const particles = useMemo<Particle[]>(() => {
    return Array.from({ length: 72 }).map((_, index) => ({
      id: index,
      x: Math.random() * 100,
      y: Math.random() * 100,
      size: 2 + Math.random() * 4,
      delay: Math.random() * 1.6,
    }));
  }, []);

  return (
    <div
      className="relative mx-auto w-[320px] h-[320px] sm:w-[360px] sm:h-[360px]"
      onMouseMove={(event) => {
        const rect = event.currentTarget.getBoundingClientRect();
        const x = event.clientX - rect.left - rect.width / 2;
        const y = event.clientY - rect.top - rect.height / 2;
        mouseX.set(x / 2.5);
        mouseY.set(y / 2.5);
      }}
      onMouseLeave={() => {
        mouseX.set(0);
        mouseY.set(0);
      }}
    >
      <motion.div
        className="absolute inset-0 rounded-full border border-border bg-secondary/30 blur-2xl"
        animate={{ scale: [0.98, 1.03, 0.98], opacity: [0.35, 0.6, 0.35] }}
        transition={{ duration: 4.5, repeat: Number.POSITIVE_INFINITY, ease: "easeInOut" }}
      />

      <motion.div
        style={{ rotateX, rotateY }}
        className="absolute inset-8 rounded-full border border-border bg-background/60 backdrop-blur-sm"
      >
        <motion.div
          className="absolute inset-[18%] rounded-full border border-border"
          animate={{ rotate: 360 }}
          transition={{ duration: 16, repeat: Number.POSITIVE_INFINITY, ease: "linear" }}
        />

        <motion.div
          className="absolute left-1/2 top-[42%] -translate-x-1/2 w-12 h-1 bg-foreground/80"
          animate={{ width: [42, 56, 42], opacity: [0.6, 1, 0.6] }}
          transition={{ duration: 2.8, repeat: Number.POSITIVE_INFINITY, ease: "easeInOut" }}
        />

        <div className="absolute left-1/2 top-[52%] -translate-x-1/2 flex items-center gap-4">
          <motion.span
            className="w-2.5 h-2.5 rounded-full bg-foreground"
            animate={{ scale: [1, 1.35, 1], opacity: [0.65, 1, 0.65] }}
            transition={{ duration: 1.4, repeat: Number.POSITIVE_INFINITY, ease: "easeInOut" }}
          />
          <motion.span
            className="w-2.5 h-2.5 rounded-full bg-foreground"
            animate={{ scale: [1, 1.35, 1], opacity: [0.65, 1, 0.65] }}
            transition={{ duration: 1.4, repeat: Number.POSITIVE_INFINITY, ease: "easeInOut", delay: 0.25 }}
          />
        </div>
      </motion.div>

      <div className="absolute inset-0 pointer-events-none">
        {particles.map((particle) => (
          <motion.span
            key={particle.id}
            className="absolute rounded-full bg-foreground/70"
            style={{
              left: `${particle.x}%`,
              top: `${particle.y}%`,
              width: `${particle.size}px`,
              height: `${particle.size}px`,
            }}
            animate={{ opacity: [0.2, 0.8, 0.2], scale: [0.8, 1.15, 0.8] }}
            transition={{
              duration: 2 + Math.random() * 1.8,
              repeat: Number.POSITIVE_INFINITY,
              ease: "easeInOut",
              delay: particle.delay,
            }}
          />
        ))}
      </div>
    </div>
  );
}
