import { useEffect, useRef, useState } from "react";

interface NumberTickerProps {
  value: number;
  className?: string;
  locale?: string;
}

export function NumberTicker({ value, className = "", locale = "en-US" }: NumberTickerProps) {
  const [displayValue, setDisplayValue] = useState(0);
  const [hasAnimated, setHasAnimated] = useState(false);
  const ref = useRef<HTMLSpanElement>(null);

  // Intersection Observer to trigger animation on viewport entry
  useEffect(() => {
    const element = ref.current;
    if (!element) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting && !hasAnimated) {
          setHasAnimated(true);
        }
      },
      { threshold: 0.1 }
    );

    observer.observe(element);
    return () => observer.disconnect();
  }, [hasAnimated]);

  // Count-up animation
  useEffect(() => {
    if (!hasAnimated || value === 0) return;

    const duration = 2000; // 2 seconds
    const startTime = performance.now();

    function animate(currentTime: number) {
      const elapsed = currentTime - startTime;
      const progress = Math.min(elapsed / duration, 1);

      // Ease out cubic for smooth deceleration
      const easedProgress = 1 - Math.pow(1 - progress, 3);
      setDisplayValue(Math.floor(easedProgress * value));

      if (progress < 1) {
        requestAnimationFrame(animate);
      } else {
        setDisplayValue(value);
      }
    }

    requestAnimationFrame(animate);
  }, [hasAnimated, value]);

  const formattedValue = displayValue.toLocaleString(locale);

  return (
    <span ref={ref} className={className}>
      {formattedValue}
    </span>
  );
}
