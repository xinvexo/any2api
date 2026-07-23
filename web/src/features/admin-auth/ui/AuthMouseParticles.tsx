import { useEffect, useRef } from "react";

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  ax: number;
  ay: number;
  life: number;
  maxLife: number;
  size: number;
  r: number;
  g: number;
  b: number;
}

const PALETTE_LIGHT = [
  [0, 113, 227],
  [94, 92, 230],
  [50, 180, 200],
  [64, 140, 255],
] as const;

const PALETTE_DARK = [
  [10, 132, 255],
  [120, 110, 240],
  [50, 215, 210],
  [90, 170, 255],
] as const;

const MAX_PARTICLES = 180;

/**
 * Canvas particles that burst outward from the cursor with random
 * directions, speeds, and curved drift — not fixed linear tracks.
 */
export function AuthMouseParticles() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    const host = canvas?.parentElement;
    if (!canvas || !host) {
      return;
    }

    const prefersReducedMotion =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (prefersReducedMotion) {
      return;
    }

    const ctx = canvas.getContext("2d");
    if (!ctx) {
      return;
    }

    const particles: Particle[] = [];
    let width = 0;
    let height = 0;
    let dpr = 1;
    let frameId = 0;
    let lastEmitAt = 0;
    let lastX = 0;
    let lastY = 0;
    let hasPointer = false;

    function resize() {
      dpr = Math.min(window.devicePixelRatio || 1, 2);
      const rect = host!.getBoundingClientRect();
      width = rect.width;
      height = rect.height;
      canvas!.width = Math.max(1, Math.floor(width * dpr));
      canvas!.height = Math.max(1, Math.floor(height * dpr));
      canvas!.style.width = `${width}px`;
      canvas!.style.height = `${height}px`;
      ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    function palette() {
      return document.documentElement.dataset.theme === "dark" ? PALETTE_DARK : PALETTE_LIGHT;
    }

    function emit(x: number, y: number, burst: number) {
      const colors = palette();
      for (let i = 0; i < burst; i += 1) {
        const angle = Math.random() * Math.PI * 2;
        const speed = 0.35 + Math.random() * 2.6;
        // Per-particle random spin so paths curve differently.
        const spin = (Math.random() - 0.5) * 0.12;
        const color = colors[Math.floor(Math.random() * colors.length)] ?? colors[0];
        particles.push({
          x: x + (Math.random() - 0.5) * 6,
          y: y + (Math.random() - 0.5) * 6,
          vx: Math.cos(angle) * speed,
          vy: Math.sin(angle) * speed,
          ax: Math.cos(angle + Math.PI / 2) * spin,
          ay: Math.sin(angle + Math.PI / 2) * spin,
          life: 0,
          maxLife: 28 + Math.random() * 55,
          size: 0.7 + Math.random() * 2.1,
          r: color[0],
          g: color[1],
          b: color[2],
        });
      }
      if (particles.length > MAX_PARTICLES) {
        particles.splice(0, particles.length - MAX_PARTICLES);
      }
    }

    function pointerPosition(event: PointerEvent) {
      const rect = host!.getBoundingClientRect();
      return {
        x: event.clientX - rect.left,
        y: event.clientY - rect.top,
      };
    }

    function onPointerMove(event: PointerEvent) {
      const { x, y } = pointerPosition(event);
      const now = performance.now();
      const moved = hasPointer
        ? Math.hypot(x - lastX, y - lastY)
        : 8;
      hasPointer = true;
      lastX = x;
      lastY = y;

      // Throttle by time and require real movement so idle hover is quiet.
      if (now - lastEmitAt < 18 || moved < 0.6) {
        return;
      }
      lastEmitAt = now;
      const burst = moved > 14 ? 3 + Math.floor(Math.random() * 3) : 1 + Math.floor(Math.random() * 2);
      emit(x, y, burst);
    }

    function onPointerLeave() {
      hasPointer = false;
    }

    function tick() {
      ctx!.clearRect(0, 0, width, height);

      for (let i = particles.length - 1; i >= 0; i -= 1) {
        const p = particles[i];
        if (!p) {
          continue;
        }
        p.life += 1;
        if (p.life >= p.maxLife) {
          particles.splice(i, 1);
          continue;
        }

        // Random micro-jitter keeps trajectories from looking like straight rays.
        p.vx += p.ax + (Math.random() - 0.5) * 0.09;
        p.vy += p.ay + (Math.random() - 0.5) * 0.09;
        p.ax *= 0.96;
        p.ay *= 0.96;
        p.vx *= 0.985;
        p.vy *= 0.985;
        p.x += p.vx;
        p.y += p.vy;

        const t = p.life / p.maxLife;
        const fade = t < 0.15 ? t / 0.15 : 1 - (t - 0.15) / 0.85;
        const alpha = Math.max(0, fade) * 0.55;
        const radius = p.size * (1 - t * 0.35);

        ctx!.beginPath();
        ctx!.arc(p.x, p.y, radius, 0, Math.PI * 2);
        ctx!.fillStyle = `rgba(${p.r}, ${p.g}, ${p.b}, ${alpha})`;
        ctx!.fill();
      }

      frameId = window.requestAnimationFrame(tick);
    }

    resize();
    host.addEventListener("pointermove", onPointerMove, { passive: true });
    host.addEventListener("pointerleave", onPointerLeave);
    window.addEventListener("resize", resize);
    frameId = window.requestAnimationFrame(tick);

    return () => {
      host.removeEventListener("pointermove", onPointerMove);
      host.removeEventListener("pointerleave", onPointerLeave);
      window.removeEventListener("resize", resize);
      window.cancelAnimationFrame(frameId);
    };
  }, []);

  return <canvas ref={canvasRef} className="auth-fx auth-fx-cursor-particles" aria-hidden="true" />;
}
