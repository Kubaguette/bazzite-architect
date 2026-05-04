/**
 * src/components/FeaturedCarousel.tsx
 *
 * Small self-contained carousel used on the dashboard to highlight featured
 * stacks/templates. Keeps behavior minimal:
 * - auto-advance at `intervalMs` when the document is visible
 * - honors an external `running` flag to pause rotation
 * - reports selection with onSelect(stackKey)
 *
 * Props:
 * - intervalMs?: rotation interval in milliseconds (default 3000)
 * - onSelect?: callback when a slide is clicked, receives the stack key
 * - running?: external pause control
 */

import { useEffect, useMemo, useState } from "react";
import CPP_Banner from "../assets/CPP_Banner.png";
import JSTS_Banner from "../assets/JSTS_Banner.png";
import Java_Banner from "../assets/Java_Banner.png";
import CSharp_Banner from "../assets/CSharp_Banner.png";
import Python_Banner from "../assets/Python_Banner.png";
import Rust_Banner from "../assets/Rust_Banner.png";

interface FeaturedCarouselProps {
  intervalMs?: number;
  onSelect?: (stack: string) => void;
  running?: boolean; // external pause control
}

const stacks = [
  { key: "react-ts", img: JSTS_Banner, alt: "TypeScript / React Stack" },
  { key: "python", img: Python_Banner, alt: "Python Stack" },
  { key: "cpp", img: CPP_Banner, alt: "C++ Stack" },
  { key: "rust", img: Rust_Banner, alt: "Rust Stack" },
  { key: "csharp", img: CSharp_Banner, alt: "C# Stack" },
  { key: "java", img: Java_Banner, alt: "Java Stack" },
];

export default function FeaturedCarousel({ intervalMs = 3000, onSelect, running = true }: FeaturedCarouselProps) {
  const [index, setIndex] = useState(0);
  const slides = useMemo(() => stacks, []);

  // Pause when tab is not visible or when running=false
  useEffect(() => {
    let visible = typeof document !== "undefined" ? !document.hidden : true;
    const onVis = () => { visible = !document.hidden; };
    document.addEventListener("visibilitychange", onVis);
    let id: number | null = null;
    const start = () => {
      if (id != null) clearInterval(id);
      if (running && visible) {
        id = window.setInterval(() => {
          setIndex((prev) => (prev + 1) % slides.length);
        }, intervalMs);
      }
    };
    start();
    return () => { if (id != null) clearInterval(id); document.removeEventListener("visibilitychange", onVis); };
  }, [slides.length, intervalMs, running]);

  const handleClick = () => {
    if (onSelect) onSelect(slides[index].key);
  };

  return (
    <div className="featured-carousel" data-tauri-drag-region="none" onClick={handleClick} role="button" aria-label="Featured Stacks">
      <div className="featured-viewport">
        {slides.map((s, i) => (
          <div key={s.key} className={`carousel-slide ${i === index ? "active" : ""}`}>
            <img src={s.img} alt={s.alt} className="carousel-image" draggable={false} />
          </div>
        ))}
      </div>
    </div>
  );
}
