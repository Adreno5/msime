import type { InputHTMLAttributes, ReactNode } from "react";

export interface LabeledInputFieldProps extends Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "value" | "onChange" | "aria-label" | "className"
> {
  label: ReactNode;
  ariaLabel: string;
  value: string;
  className?: string;
  labelClassName?: string;
  onChange: (value: string) => void;
}

/** Shared controlled text input primitive used by account and settings forms. */
export function LabeledInputField({
  label,
  ariaLabel,
  value,
  className,
  labelClassName,
  onChange,
  ...inputProps
}: LabeledInputFieldProps) {
  return (
    <label className={labelClassName}>
      {label}
      <input
        {...inputProps}
        className={className}
        aria-label={ariaLabel}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}
