import { useId } from "react";
import { cn } from "./cn";

/**
 * A labeled one-line text input, styled after shadcn/ui's Input and Label.
 * @public
 */
export function TextField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const id = useId();

  return (
    <div className="flex flex-1 flex-col gap-2">
      <label htmlFor={id} className="text-sm leading-none font-medium">
        {label}
      </label>
      <input
        id={id}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className={cn(
          "h-9 w-full min-w-0 px-3 py-1",
          "placeholder:text-muted-foreground text-base md:text-sm",
          "border-input rounded-md border bg-transparent shadow-xs",
          "transition-[color,box-shadow] outline-none",
          "focus-visible:border-ring",
          "focus-visible:ring-ring/50 focus-visible:ring-[3px]",
        )}
      />
    </div>
  );
}
