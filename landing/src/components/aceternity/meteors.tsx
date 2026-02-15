import { cn } from "@/lib/utils";
import { useMemo } from "react";

export interface MeteorsProps {
  number?: number;
  className?: string;
}

export const Meteors = ({ number = 20, className }: MeteorsProps) => {
  const meteors = useMemo(() => {
    return new Array(number).fill(true).map((_, idx) => ({
      id: idx,
      top: Math.floor(Math.random() * 100) - 20,
      left: Math.floor(Math.random() * 100),
      delay: Math.random() * 4,
      duration: Math.floor(Math.random() * 5 + 3),
    }));
  }, [number]);

  return (
    <div className="absolute inset-0 overflow-hidden pointer-events-none z-0">
      {meteors.map((meteor) => (
        <span
          key={"meteor" + meteor.id}
          className={cn("meteor-el", className)}
          style={{
            top: `${meteor.top}%`,
            left: `${meteor.left}%`,
            animationDelay: `${meteor.delay}s`,
            animationDuration: `${meteor.duration}s`,
          }}
        />
      ))}
    </div>
  );
};
